// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::Client;
use crate::types::Role;
use grammers_mtsender::{InvocationError, RpcError};
use grammers_session::PackedChat;
use grammers_tl_types as tl;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    marker::PhantomPinned,
    mem::drop,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use web_time::{SystemTime, UNIX_EPOCH};

type BuilderRes = Result<(), InvocationError>;
type AdminFutGen<F> = fn(AdminRightsBuilderInner) -> F;

pub(crate) struct AdminRightsBuilderInner {
    client: Client,
    chat: PackedChat,
    peer: tl::enums::InputPeer,
    user: tl::enums::InputUser, // TODO redundant with `peer` (but less annoying to use)
    rights: tl::types::ChatAdminRights,
    rank: String,
}

impl AdminRightsBuilderInner {
    // Perform the call.
    pub(crate) async fn invoke(self) -> Result<(), InvocationError> {
        if let Some(chan) = self.chat.try_to_input_channel() {
            self.client
                .invoke(&tl::functions::channels::EditAdmin {
                    channel: chan,
                    user_id: self.user.clone(),
                    admin_rights: tl::enums::ChatAdminRights::Rights(self.rights.clone()),
                    rank: self.rank.clone(),
                })
                .await
                .map(drop)
        } else if let Some(id) = self.chat.try_to_chat_id() {
            let promote = self.rights.anonymous
                || self.rights.change_info
                || self.rights.post_messages
                || self.rights.edit_messages
                || self.rights.delete_messages
                || self.rights.ban_users
                || self.rights.invite_users
                || self.rights.pin_messages
                || self.rights.add_admins
                || self.rights.manage_call;
            self.client
                .invoke(&tl::functions::messages::EditChatAdmin {
                    chat_id: id,
                    user_id: self.user.clone(),
                    is_admin: promote,
                })
                .await
                .map(drop)
        } else {
            Err(InvocationError::Rpc(RpcError {
                code: 400,
                name: "PEER_ID_INVALID".to_string(),
                value: None,
                caused_by: None,
            }))
        }
    }
}

pin_project! {
    /// Builder for editing the administrator rights of a user in a specific chat.
    ///
    /// Use [`Client::set_admin_rights`] to retrieve an instance of this type.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct AdminRightsBuilder<F: Future<Output = BuilderRes>> {
        inner: Option<AdminRightsBuilderInner>,
        fut_gen: AdminFutGen<F>,
        #[pin]
        fut: Option<F>,
        _phantom: PhantomPinned
    }
}

impl<F: Future<Output = BuilderRes>> Future for AdminRightsBuilder<F> {
    type Output = BuilderRes;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<BuilderRes> {
        let mut s = self.project();
        if s.fut.is_none() {
            // unwrap safety: s.inner is None only when s.fut is some
            // or s.fut is resolved
            s.fut.set(Some((s.fut_gen)(s.inner.take().unwrap())))
        }

        s.fut.as_pin_mut().unwrap().poll(cx)
    }
}

impl<F: Future<Output = BuilderRes>> AdminRightsBuilder<F> {
    pub(crate) fn new(
        client: Client,
        chat: PackedChat,
        user: PackedChat,
        fut_gen: AdminFutGen<F>,
    ) -> Self {
        Self {
            inner: Some(AdminRightsBuilderInner {
                client,
                chat,
                peer: user.to_input_peer(),
                user: user.to_input_user_lossy(),
                rank: "".into(),
                rights: tl::types::ChatAdminRights {
                    anonymous: false,
                    change_info: false,
                    post_messages: false,
                    edit_messages: false,
                    delete_messages: false,
                    ban_users: false,
                    invite_users: false,
                    pin_messages: false,
                    add_admins: false,
                    manage_call: false,
                    other: false,
                    manage_topics: false,
                    post_stories: false,
                    edit_stories: false,
                    delete_stories: false,
                    manage_direct_messages: false,
                },
            }),
            fut_gen,
            fut: None,
            _phantom: PhantomPinned,
        }
    }

    fn inner_mut(&mut self) -> &mut AdminRightsBuilderInner {
        // Unwrap safety: AdminRightsBuilderInner should never be None unless polled after being
        // resolved
        self.inner.as_mut().unwrap()
    }

    /// Load the current rights of the user. This lets you trivially grant or take away specific
    /// permissions without changing any of the previous ones.
    pub async fn load_current(mut self) -> Result<Self, InvocationError> {
        let s = self.inner_mut();
        if let Some(chan) = s.chat.try_to_input_channel() {
            let tl::enums::channels::ChannelParticipant::Participant(user) = s
                .client
                .invoke(&tl::functions::channels::GetParticipant {
                    channel: chan,
                    participant: s.peer.clone(),
                })
                .await?;
            match user.participant {
                tl::enums::ChannelParticipant::Creator(c) => {
                    s.rights = c.admin_rights.into();
                    s.rank = c.rank.unwrap_or_default();
                }
                tl::enums::ChannelParticipant::Admin(a) => {
                    s.rights = a.admin_rights.into();
                    s.rank = a.rank.unwrap_or_default();
                }
                _ => (),
            }
        } else if s.chat.try_to_chat_id().is_some() {
            let uid = match &s.user {
                tl::enums::InputUser::User(u) => u.user_id,
                tl::enums::InputUser::FromMessage(u) => u.user_id,
                _ => {
                    return Err(InvocationError::Rpc(RpcError {
                        code: 400,
                        name: "PEER_ID_INVALID".to_string(),
                        value: None,
                        caused_by: None,
                    }));
                }
            };

            let mut participants = s.client.iter_participants(s.chat);
            while let Some(participant) = participants.next().await? {
                if matches!(participant.role, Role::Creator(_) | Role::Admin(_))
                    && participant.user.id() == uid
                {
                    s.rights = tl::types::ChatAdminRights {
                        change_info: true,
                        post_messages: true,
                        edit_messages: false,
                        delete_messages: true,
                        ban_users: true,
                        invite_users: true,
                        pin_messages: true,
                        add_admins: true,
                        anonymous: false,
                        manage_call: true,
                        other: true,
                        manage_topics: true,
                        post_stories: true,
                        edit_stories: true,
                        delete_stories: true,
                        manage_direct_messages: true,
                    };
                    break;
                }
            }
        }

        Ok(self)
    }

    /// Whether the user will remain anonymous when sending messages.
    ///
    /// The sender of the anonymous messages becomes the group itself.
    ///
    /// Note that other people in the channel may be able to identify the anonymous admin by its
    /// custom rank, so additional care is needed when using both anonymous and custom ranks.
    ///
    /// For example, if multiple anonymous admins share the same title, users won't be able to
    /// distinguish them.
    pub fn anonymous(mut self, val: bool) -> Self {
        self.inner_mut().rights.anonymous = val;
        self
    }

    /// Whether the user is able to manage calls in the group.
    pub fn manage_call(mut self, val: bool) -> Self {
        self.inner_mut().rights.manage_call = val;
        self
    }

    /// Whether the user is able to change information about the chat such as group description or
    /// not.
    pub fn change_info(mut self, val: bool) -> Self {
        self.inner_mut().rights.change_info = val;
        self
    }

    /// Whether the user will be able to post in the channel. This will only work in broadcast
    /// channels, not groups.
    pub fn post_messages(mut self, val: bool) -> Self {
        self.inner_mut().rights.post_messages = val;
        self
    }

    /// Whether the user will be able to edit messages in the channel. This will only work in
    /// broadcast channels, not groups.
    pub fn edit_messages(mut self, val: bool) -> Self {
        self.inner_mut().rights.edit_messages = val;
        self
    }

    /// Whether the user will be able to delete messages. This includes messages from others.
    pub fn delete_messages(mut self, val: bool) -> Self {
        self.inner_mut().rights.delete_messages = val;
        self
    }

    /// Whether the user will be able to edit the restrictions of other users. This effectively
    /// lets the administrator ban (or kick) people.
    pub fn ban_users(mut self, val: bool) -> Self {
        self.inner_mut().rights.ban_users = val;
        self
    }

    /// Whether the user will be able to invite other users.
    pub fn invite_users(mut self, val: bool) -> Self {
        self.inner_mut().rights.invite_users = val;
        self
    }

    /// Whether the user will be able to pin messages.
    pub fn pin_messages(mut self, val: bool) -> Self {
        self.inner_mut().rights.pin_messages = val;
        self
    }

    /// Whether the user will be able to add other administrators with the same or less
    /// permissions than the user itself.
    pub fn add_admins(mut self, val: bool) -> Self {
        self.inner_mut().rights.add_admins = val;
        self
    }

    /// The custom rank  (also known as "admin title" or "badge") to show for this administrator.
    ///
    /// This text will be shown instead of the "admin" badge.
    ///
    /// When left unspecified or empty, the default localized "admin" badge will be shown.
    pub fn rank<S: Into<String>>(mut self, val: S) -> Self {
        self.inner_mut().rank = val.into();
        self
    }
}

type BannedFutGen<F> = fn(BannedRightsBuilderInner) -> F;

pub(crate) struct BannedRightsBuilderInner {
    client: Client,
    chat: PackedChat,
    peer: tl::enums::InputPeer,
    user: tl::enums::InputUser,
    rights: tl::types::ChatBannedRights,
}

impl BannedRightsBuilderInner {
    // Perform the call.
    pub(crate) async fn invoke(self) -> Result<(), InvocationError> {
        if let Some(chan) = self.chat.try_to_input_channel() {
            self.client
                .invoke(&tl::functions::channels::EditBanned {
                    channel: chan,
                    participant: self.peer.clone(),
                    banned_rights: tl::enums::ChatBannedRights::Rights(self.rights.clone()),
                })
                .await
                .map(drop)
        } else if let Some(id) = self.chat.try_to_chat_id() {
            if self.rights.view_messages {
                self.client
                    .invoke(&tl::functions::messages::DeleteChatUser {
                        chat_id: id,
                        user_id: self.user.clone(),
                        revoke_history: false,
                    })
                    .await
                    .map(drop)
            } else {
                Err(InvocationError::Rpc(RpcError {
                    code: 400,
                    name: "CHAT_INVALID".to_string(),
                    value: None,
                    caused_by: None,
                }))
            }
        } else {
            Err(InvocationError::Rpc(RpcError {
                code: 400,
                name: "PEER_ID_INVALID".to_string(),
                value: None,
                caused_by: None,
            }))
        }
    }
}

pin_project! {
    /// Builder for editing the rights of a non-admin user in a specific chat.
    ///
    /// Certain groups (small group chats) only allow banning (disallow `view_messages`). Trying to
    /// disallow other permissions in these groups will fail.
    ///
    /// Use [`Client::set_banned_rights`] to retrieve an instance of this type.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct BannedRightsBuilder<F: Future<Output = BuilderRes>> {
        inner: Option<BannedRightsBuilderInner>,
        fut_gen: BannedFutGen<F>,
        #[pin]
        fut: Option<F>,
        _phantom: PhantomPinned
    }
}

impl<F: Future<Output = BuilderRes>> Future for BannedRightsBuilder<F> {
    type Output = BuilderRes;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<BuilderRes> {
        let mut s = self.project();
        if s.fut.is_none() {
            // unwrap safety: s.inner is None only when s.fut is some
            // or s.fut is resolved
            s.fut.set(Some((s.fut_gen)(s.inner.take().unwrap())))
        }

        s.fut.as_pin_mut().unwrap().poll(cx)
    }
}

impl<F: Future<Output = BuilderRes>> BannedRightsBuilder<F> {
    pub(crate) fn new(
        client: Client,
        chat: PackedChat,
        user: PackedChat,
        fut_gen: BannedFutGen<F>,
    ) -> Self {
        Self {
            inner: Some(BannedRightsBuilderInner {
                client,
                chat,
                peer: user.to_input_peer(),
                user: user.to_input_user_lossy(),
                rights: tl::types::ChatBannedRights {
                    view_messages: false,
                    send_messages: false,
                    send_media: false,
                    send_stickers: false,
                    send_gifs: false,
                    send_games: false,
                    send_inline: false,
                    embed_links: false,
                    send_polls: false,
                    change_info: false,
                    invite_users: false,
                    pin_messages: false,
                    manage_topics: false,
                    send_photos: false,
                    send_videos: false,
                    send_roundvideos: false,
                    send_audios: false,
                    send_voices: false,
                    send_docs: false,
                    send_plain: false,
                    until_date: 0,
                },
            }),
            fut_gen,
            fut: None,
            _phantom: PhantomPinned,
        }
    }

    fn inner_mut(&mut self) -> &mut BannedRightsBuilderInner {
        // Unwrap safety: AdminRightsBuilderInner should never be None unless polled after being
        // resolved
        self.inner.as_mut().unwrap()
    }

    /// Load the current rights of the user. This lets you trivially grant or take away specific
    /// permissions without changing any of the previous ones.
    pub async fn load_current(mut self) -> Result<Self, InvocationError> {
        let s = self.inner_mut();
        if let Some(chan) = s.chat.try_to_input_channel() {
            let tl::enums::channels::ChannelParticipant::Participant(user) = s
                .client
                .invoke(&tl::functions::channels::GetParticipant {
                    channel: chan,
                    participant: s.peer.clone(),
                })
                .await?;

            if let tl::enums::ChannelParticipant::Banned(u) = user.participant {
                s.rights = u.banned_rights.into();
            }
        }

        Ok(self)
    }

    /// Whether the user is able to view messages or not. Forbidding someone from viewing messages
    /// effectively bans (kicks) them.
    pub fn view_messages(mut self, val: bool) -> Self {
        // `true` indicates "take away", but in the builder it makes more sense that `false` means
        // "they won't have this permission". All methods perform this negation for that reason.
        self.inner_mut().rights.view_messages = !val;
        self
    }

    /// Whether the user is able to send messages or not. The user will remain in the chat, and
    /// can still read the conversation.
    pub fn send_messages(mut self, val: bool) -> Self {
        self.inner_mut().rights.send_messages = !val;
        self
    }

    /// Whether the user is able to send any form of media or not, such as photos or voice notes.
    pub fn send_media(mut self, val: bool) -> Self {
        self.inner_mut().rights.send_media = !val;
        self
    }

    /// Whether the user is able to send stickers or not.
    pub fn send_stickers(mut self, val: bool) -> Self {
        self.inner_mut().rights.send_stickers = !val;
        self
    }

    /// Whether the user is able to send animated gifs or not.
    pub fn send_gifs(mut self, val: bool) -> Self {
        self.inner_mut().rights.send_gifs = !val;
        self
    }

    /// Whether the user is able to send games or not.
    pub fn send_games(mut self, val: bool) -> Self {
        self.inner_mut().rights.send_games = !val;
        self
    }

    /// Whether the user is able to use inline bots or not.
    pub fn send_inline(mut self, val: bool) -> Self {
        self.inner_mut().rights.send_inline = !val;
        self
    }

    /// Whether the user is able to enable the link preview in the messages they send.
    ///
    /// Note that the user will still be able to send messages with links if this permission is
    /// taken away from the user, but these links won't display a link preview.
    pub fn embed_link_previews(mut self, val: bool) -> Self {
        self.inner_mut().rights.embed_links = !val;
        self
    }

    /// Whether the user is able to send polls or not.
    pub fn send_polls(mut self, val: bool) -> Self {
        self.inner_mut().rights.send_polls = !val;
        self
    }

    /// Whether the user is able to change information about the chat such as group description or
    /// not.
    pub fn change_info(mut self, val: bool) -> Self {
        self.inner_mut().rights.change_info = !val;
        self
    }

    /// Whether the user is able to invite other users or not.
    pub fn invite_users(mut self, val: bool) -> Self {
        self.inner_mut().rights.invite_users = !val;
        self
    }

    /// Whether the user is able to pin messages or not.
    pub fn pin_messages(mut self, val: bool) -> Self {
        self.inner_mut().rights.pin_messages = !val;
        self
    }

    /// Apply the restrictions until the given epoch time.
    ///
    /// Note that this is absolute time (i.e current time is not added).
    ///
    /// By default, the restriction is permanent.
    pub fn until(mut self, val: i32) -> Self {
        // TODO this should take a date, not int
        self.inner_mut().rights.until_date = val;
        self
    }

    /// Apply the restriction for a given duration.
    pub fn duration(mut self, val: Duration) -> Self {
        // TODO this should account for the server time instead (via sender's offset)
        self.inner_mut().rights.until_date = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before epoch")
            .as_secs() as i32
            + val.as_secs() as i32;

        self
    }
}
