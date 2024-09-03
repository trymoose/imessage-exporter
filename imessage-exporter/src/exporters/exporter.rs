use std::{fs::File, io::BufWriter, marker::Sized};

use imessage_database::{
    error::{plist::PlistParseError, table::TableError},
    message_types::{
        app::AppMessage,
        app_store::AppStoreMessage,
        collaboration::CollaborationMessage,
        edited::EditedMessage,
        handwriting::HandwrittenMessage,
        music::MusicMessage,
        placemark::PlacemarkMessage,
        text_effects::{Animation, Style, TextEffect, Unit},
        url::URLMessage,
    },
    tables::{attachment::Attachment, messages::Message},
};

use crate::app::{error::RuntimeError, runtime::Config};

/// Defines behavior for iterating over messages from the iMessage database and managing export files
pub trait Exporter<'a> {
    /// Create a new exporter with references to the cached data
    fn new(config: &'a Config) -> Result<Self, RuntimeError>
    where
        Self: Sized;
    /// Begin iterating over the messages table
    fn iter_messages(&mut self) -> Result<(), RuntimeError>;
    /// Get the file handle to write to, otherwise create a new one
    fn get_or_create_file(
        &mut self,
        message: &Message,
    ) -> Result<&mut BufWriter<File>, RuntimeError>;
}

/// Defines behavior for formatting message instances to the desired output format
pub(super) trait Writer<'a, R> {
    /// Format a message, including its reactions and replies
    fn format_message(&self, msg: &Message, indent: usize) -> Result<R, TableError>;
    /// Format an attachment, possibly by reading the disk
    fn format_attachment(
        &self,
        attachment: &'a mut Attachment,
        msg: &'a Message,
    ) -> Result<R, &'a str>;
    /// Format a sticker, possibly by reading the disk
    fn format_sticker(&self, attachment: &'a mut Attachment, msg: &'a Message) -> R;
    /// Format an app message by parsing some of its fields
    fn format_app(
        &self,
        msg: &'a Message,
        attachments: &mut Vec<Attachment>,
        indent: &str,
    ) -> Result<R, PlistParseError>;
    /// Format a reaction (displayed under a message)
    fn format_reaction(&self, msg: &Message) -> Result<R, TableError>;
    /// Format an expressive message
    fn format_expressive(&self, msg: &'a Message) -> R;
    /// Format an announcement message
    fn format_announcement(&self, msg: &'a Message) -> R;
    /// Format a `SharePlay` message
    fn format_shareplay(&self) -> R;
    /// Format a legacy Shared Location message
    fn format_shared_location(&self, msg: &'a Message) -> R;
    /// Format an edited message
    fn format_edited(
        &self,
        msg: &'a Message,
        edited_message: &'a EditedMessage,
        message_part_idx: usize,
        indent: &str,
    ) -> Option<R>;
    /// Format some attributed text
    fn format_attributed(&'a self, text: &'a str, attribute: &'a TextEffect) -> R;
    fn write_to_file(file: &mut BufWriter<File>, text: R) -> Result<(), RuntimeError>;
}

/// Defines behavior for formatting custom balloons to the desired output format
pub(super) trait BalloonFormatter<T, R> {
    /// Format a URL message
    fn format_url(&self, balloon: &URLMessage, indent: T) -> R;
    /// Format an Apple Music message
    fn format_music(&self, balloon: &MusicMessage, indent: T) -> R;
    /// Format a Rich Collaboration message
    fn format_collaboration(&self, balloon: &CollaborationMessage, indent: T) -> R;
    /// Format an App Store link
    fn format_app_store(&self, balloon: &AppStoreMessage, indent: T) -> R;
    /// Format a shared location message
    fn format_placemark(&self, balloon: &PlacemarkMessage, indent: T) -> R;
    /// Format a handwritten note message
    fn format_handwriting(&self, balloon: &HandwrittenMessage, indent: T) -> R;
    /// Format an Apple Pay message
    fn format_apple_pay(&self, balloon: &AppMessage, indent: T) -> R;
    /// Format a Fitness message
    fn format_fitness(&self, balloon: &AppMessage, indent: T) -> R;
    /// Format a Photo Slideshow message
    fn format_slideshow(&self, balloon: &AppMessage, indent: T) -> R;
    /// Format a Find My message
    fn format_find_my(&self, balloon: &AppMessage, indent: T) -> R;
    /// Format a Check In message
    fn format_check_in(&self, balloon: &AppMessage, indent: T) -> R;
    /// Format a generic app, generally third party
    fn format_generic_app(
        &self,
        balloon: &AppMessage,
        bundle_id: &str,
        attachments: &mut Vec<Attachment>,
        indent: T,
    ) -> R;
}

pub(super) trait TextEffectFormatter<R> {
    /// Format message text containing a [`Mention`](imessage_database::message_types::text_effects::TextEffect::Mention)
    fn format_mention(&self, text: &str, mentioned: &str) -> R;
    /// Format message text containing a [`Link`](imessage_database::message_types::text_effects::TextEffect::Link)
    fn format_link(&self, text: &str, url: &str) -> R;
    /// Format message text containing an [`OTP`](imessage_database::message_types::text_effects::TextEffect::OTP)
    fn format_otp(&self, text: &str) -> R;
    /// Format message text containing a [`Conversion`](imessage_database::message_types::text_effects::TextEffect::Conversion)
    fn format_conversion(&self, text: &str, unit: &Unit) -> R;
    /// Format message text containing some [`Styles`](imessage_database::message_types::text_effects::TextEffect::Styles)
    fn format_styles(&self, text: &str, styles: &[Style]) -> R;
    /// Format [`Animated`](imessage_database::message_types::text_effects::TextEffect::Animated) message text
    fn format_animated(&self, text: &str, animation: &Animation) -> R;
}
