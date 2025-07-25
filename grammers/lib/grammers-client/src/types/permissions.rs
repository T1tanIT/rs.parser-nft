// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::utils;
use chrono::{DateTime, Utc};
use grammers_tl_types as tl;

#[derive(Clone, Debug, PartialEq)]
pub struct Permissions {
    pub raw: tl::types::ChatAdminRights,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Restrictions {
    pub raw: tl::types::ChatBannedRights,
}

impl Permissions {
    pub(crate) fn new_full() -> Self {
        Self {
            raw: tl::types::ChatAdminRights {
                change_info: true,
                post_messages: true,
                edit_messages: true,
                delete_messages: true,
                ban_users: true,
                invite_users: true,
                pin_messages: true,
                add_admins: true,
                anonymous: true,
                manage_call: true,
                other: true,
                manage_topics: true,
                post_stories: true,
                edit_stories: true,
                delete_stories: true,
                manage_direct_messages: false,
            },
        }
    }

    pub(crate) fn from_raw(rights: tl::types::ChatAdminRights) -> Self {
        Self { raw: rights }
    }

    pub fn change_info(&self) -> bool {
        self.raw.change_info
    }

    pub fn post_messages(&self) -> bool {
        self.raw.post_messages
    }

    pub fn edit_messages(&self) -> bool {
        self.raw.edit_messages
    }

    pub fn delete_messages(&self) -> bool {
        self.raw.delete_messages
    }

    pub fn ban_users(&self) -> bool {
        self.raw.ban_users
    }

    pub fn invite_users(&self) -> bool {
        self.raw.invite_users
    }

    pub fn pin_messages(&self) -> bool {
        self.raw.pin_messages
    }

    pub fn add_admins(&self) -> bool {
        self.raw.add_admins
    }

    pub fn anonymous(&self) -> bool {
        self.raw.anonymous
    }

    pub fn manage_call(&self) -> bool {
        self.raw.manage_call
    }
}

impl Restrictions {
    pub(crate) fn from_raw(rights: tl::types::ChatBannedRights) -> Self {
        Self { raw: rights }
    }

    pub fn view_messages(&self) -> bool {
        self.raw.view_messages
    }

    pub fn send_messages(&self) -> bool {
        self.raw.send_messages
    }

    pub fn send_media(&self) -> bool {
        self.raw.send_media
    }

    pub fn send_stickers(&self) -> bool {
        self.raw.send_stickers
    }

    pub fn send_gifs(&self) -> bool {
        self.raw.send_gifs
    }

    pub fn send_games(&self) -> bool {
        self.raw.send_games
    }

    pub fn send_inline(&self) -> bool {
        self.raw.send_inline
    }

    pub fn embed_links(&self) -> bool {
        self.raw.embed_links
    }

    pub fn send_polls(&self) -> bool {
        self.raw.send_polls
    }

    pub fn change_info(&self) -> bool {
        self.raw.change_info
    }

    pub fn invite_users(&self) -> bool {
        self.raw.invite_users
    }

    pub fn pin_messages(&self) -> bool {
        self.raw.pin_messages
    }

    pub fn due(&self) -> DateTime<Utc> {
        utils::date(self.raw.until_date)
    }
}
