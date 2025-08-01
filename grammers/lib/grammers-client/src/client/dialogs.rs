// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::Client;
use crate::types::{ChatMap, Dialog, IterBuffer};
use grammers_mtsender::InvocationError;
use grammers_session::PackedChat;
use grammers_tl_types as tl;

const MAX_LIMIT: usize = 100;

pub type DialogIter = IterBuffer<tl::functions::messages::GetDialogs, Dialog>;

impl DialogIter {
    fn new(client: &Client) -> Self {
        // TODO let users tweak all the options from the request
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::GetDialogs {
                exclude_pinned: false,
                folder_id: None,
                offset_date: 0,
                offset_id: 0,
                offset_peer: tl::enums::InputPeer::Empty,
                limit: 0,
                hash: 0,
            },
        )
    }

    /// Determines how many dialogs there are in total.
    ///
    /// This only performs a network call if `next` has not been called before.
    pub async fn total(&mut self) -> Result<usize, InvocationError> {
        if let Some(total) = self.total {
            return Ok(total);
        }

        use tl::enums::messages::Dialogs;

        self.request.limit = 1;
        let total = match self.client.invoke(&self.request).await? {
            Dialogs::Dialogs(dialogs) => dialogs.dialogs.len(),
            Dialogs::Slice(dialogs) => dialogs.count as usize,
            Dialogs::NotModified(dialogs) => dialogs.count as usize,
        };
        self.total = Some(total);
        Ok(total)
    }

    /// Return the next `Dialog` from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no dialogs left.
    pub async fn next(&mut self) -> Result<Option<Dialog>, InvocationError> {
        if let Some(result) = self.next_raw() {
            return result;
        }

        use tl::enums::messages::Dialogs;

        self.request.limit = self.determine_limit(MAX_LIMIT);
        let (dialogs, mut messages, users, chats) = match self.client.invoke(&self.request).await? {
            Dialogs::Dialogs(d) => {
                self.last_chunk = true;
                self.total = Some(d.dialogs.len());
                (d.dialogs, d.messages, d.users, d.chats)
            }
            Dialogs::Slice(d) => {
                self.last_chunk = d.dialogs.len() < self.request.limit as usize;
                self.total = Some(d.count as usize);
                (d.dialogs, d.messages, d.users, d.chats)
            }
            Dialogs::NotModified(_) => {
                panic!("API returned Dialogs::NotModified even though hash = 0")
            }
        };

        {
            let mut state = self.client.0.state.write().unwrap();
            // Telegram can return peers without hash (e.g. Users with 'min: true')
            let _ = state.chat_hashes.extend(&users, &chats);
        }

        let chats = ChatMap::new(users, chats);

        {
            let mut state = self.client.0.state.write().unwrap();
            self.buffer.extend(dialogs.into_iter().map(|dialog| {
                if let tl::enums::Dialog::Dialog(tl::types::Dialog {
                    peer: tl::enums::Peer::Channel(channel),
                    pts: Some(pts),
                    ..
                }) = &dialog
                {
                    state
                        .message_box
                        .try_set_channel_state(channel.channel_id, *pts);
                }
                Dialog::new(&self.client, dialog, &mut messages, &chats)
            }));
        }

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            self.request.exclude_pinned = true;
            if let Some(last_message) = self
                .buffer
                .iter()
                .rev()
                .find_map(|dialog| dialog.last_message.as_ref())
            {
                self.request.offset_date = last_message.date_timestamp();
                self.request.offset_id = last_message.id();
            }
            self.request.offset_peer = self.buffer[self.buffer.len() - 1]
                .chat()
                .pack()
                .to_input_peer();
        }

        Ok(self.pop_item())
    }
}

/// Method implementations related to open conversations.
impl Client {
    /// Returns a new iterator over the dialogs.
    ///
    /// While iterating, the update state for any broadcast channel or megagroup will be set if it was unknown before.
    /// When the update state is set for these chats, the library can actively check to make sure it's not missing any
    /// updates from them (as long as the queue limit for updates is larger than zero).
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut dialogs = client.iter_dialogs();
    ///
    /// while let Some(dialog) = dialogs.next().await? {
    ///     let chat = dialog.chat();
    ///     println!("{} ({})", chat.name().unwrap_or_default(), chat.id());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_dialogs(&self) -> DialogIter {
        DialogIter::new(self)
    }

    /// Deletes a dialog, effectively removing it from your list of open conversations.
    ///
    /// The dialog is only deleted for yourself.
    ///
    /// Deleting a dialog effectively clears the message history and "kicks" you from it.
    ///
    /// For groups and channels, this is the same as leaving said chat. This method does **not**
    /// delete the chat itself (the chat still exists and the other members will remain inside).
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Consider making a backup before, you will lose access to the messages in chat!
    /// client.delete_dialog(&chat).await?;
    /// # Ok(())
    /// # }
    /// ```

    pub async fn get_unique_star_gift(&self, slug: String) -> Result<tl::enums::payments::UniqueStarGift, InvocationError> {
        self.invoke(&tl::functions::payments::GetUniqueStarGift { slug })
        .await
    }
    pub async fn delete_dialog<C: Into<PackedChat>>(&self, chat: C) -> Result<(), InvocationError> {
        let chat = chat.into();
        if let Some(channel) = chat.try_to_input_channel() {
            self.invoke(&tl::functions::channels::LeaveChannel { channel })
            .await
            .map(drop)
        } else if let Some(chat_id) = chat.try_to_chat_id() {
            // TODO handle PEER_ID_INVALID and ignore it (happens when trying to delete deactivated chats)
            self.invoke(&tl::functions::messages::DeleteChatUser {
                chat_id,
                user_id: tl::enums::InputUser::UserSelf,
                revoke_history: false,
            })
            .await
            .map(drop)
        } else {
            // TODO only do this if we're not a bot
            self.invoke(&tl::functions::messages::DeleteHistory {
                just_clear: false,
                revoke: false,
                peer: chat.to_input_peer(),
                max_id: 0,
                min_date: None,
                max_date: None,
            })
            .await
            .map(drop)
        }
    }

    /// Mark a chat as read.
    ///
    /// If you want to get rid of all the mentions (for example, a voice note that you have not
    /// listened to yet), you need to also use [`Client::clear_mentions`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.mark_as_read(&chat).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn mark_as_read<C: Into<PackedChat>>(&self, chat: C) -> Result<(), InvocationError> {
        let chat = chat.into();
        if let Some(channel) = chat.try_to_input_channel() {
            self.invoke(&tl::functions::channels::ReadHistory { channel, max_id: 0 })
            .await
            .map(drop)
        } else {
            self.invoke(&tl::functions::messages::ReadHistory {
                peer: chat.to_input_peer(),
                max_id: 0,
            })
            .await
            .map(drop)
        }
    }

    /// Clears all pending mentions from a chat, marking them as read.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.clear_mentions(&chat).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn clear_mentions<C: Into<PackedChat>>(
        &self,
        chat: C,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::ReadMentions {
            peer: chat.into().to_input_peer(),
            top_msg_id: None,
        })
        .await
        .map(drop)
    }
}
