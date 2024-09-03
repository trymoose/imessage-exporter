use crate::{app::{error::RuntimeError, progress::build_progress_bar_export, runtime::Config}, exporters::exporter::{BalloonFormatter, Exporter, TextEffectFormatter, Writer}};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use json::{array, from, object, object::Object, JsonValue};

use imessage_database::message_types::text_effects::{Animation, Style, Unit};
use imessage_database::tables::table::{MESSAGES_FILE, MESSAGES_FILE_EXT};
use imessage_database::util::dates::unreadable_diff;
use imessage_database::{
    error::{plist::PlistParseError, table::TableError},
    message_types::{
        app::AppMessage,
        app_store::AppStoreMessage,
        collaboration::CollaborationMessage,
        edited::{EditStatus, EditedMessage},
        expressives::Expressive,
        handwriting::HandwrittenMessage,
        music::MusicMessage,
        placemark::PlacemarkMessage,
        text_effects::TextEffect,
        url::URLMessage,
        variants::{Announcement, BalloonProvider, CustomBalloon, URLOverride, Variant},
    },
    tables::{
        attachment::Attachment,
        messages::{models::BubbleComponent, Message},
        table::{Table, FITNESS_RECEIVER, ME, YOU},
    },
    util::{
        dates::{format, get_local_time, readable_diff, TIMESTAMP_FACTOR},
        plist::parse_plist,
    },
};

pub struct NDJSON<'a> {
    /// Data that is setup from the application's runtime
    pub config: &'a Config,
    /// Handles to file we want to write messages to
    /// Resolved messages file location to a buffered writer
    pub file: BufWriter<File>,
}

impl<'a> Exporter<'a> for NDJSON<'a> {
    fn new(config: &'a Config) -> Result<Self, RuntimeError> {
        let mut filename = config.options.export_path.clone();
        filename.push(MESSAGES_FILE);
        filename.set_extension(MESSAGES_FILE_EXT);

        let file = File::options()
            .append(true)
            .create(true)
            .open(filename)
            .map_err(RuntimeError::DiskError)?;

        Ok(NDJSON {
            config,
            file: BufWriter::new(file),
        })
    }

    fn iter_messages(&mut self) -> Result<(), RuntimeError> {
        // Tell the user what we are doing
        eprintln!(
            "Exporting to {} as ndjson...",
            self.config.options.export_path.display()
        );

        // Keep track of current message ROWID
        let mut current_message_row = -1;

        // Set up progress bar
        let mut current_message = 0;
        let total_messages =
            Message::get_count(&self.config.db, &self.config.options.query_context)
                .map_err(RuntimeError::DatabaseError)?;
        let pb = build_progress_bar_export(total_messages);

        let mut statement =
            Message::stream_rows(&self.config.db, &self.config.options.query_context)
                .map_err(RuntimeError::DatabaseError)?;

        let messages = statement
            .query_map([], |row| Ok(Message::from_row(row)))
            .map_err(|err| RuntimeError::DatabaseError(TableError::Messages(err)))?;

        for message in messages {
            let mut msg = Message::extract(message).map_err(RuntimeError::DatabaseError)?;

            // Early escape if we try and render the same message GUID twice
            // See https://github.com/ReagentX/imessage-exporter/issues/135 for rationale
            if msg.rowid == current_message_row {
                current_message += 1;
                continue;
            }
            current_message_row = msg.rowid;

            // Generate the text of the message
            let _ = msg.generate_text(&self.config.db);

            // Render the announcement in-line
            if msg.is_announcement() {
                let chatroom = self.get_chatroom(&msg);
                let announcement = self.format_announcement(&msg);
                NDJSON::write_to_file(self.get_or_create_file(&msg)?, object!{
                    chatroom: chatroom,
                    announcement: announcement,
                })?;
            }
            // Message replies and reactions are rendered in context, so no need to render them separately
            else if !msg.is_reaction() {
                let chatroom = self.get_chatroom(&msg);
                let message = self
                    .format_message(&msg, 0)
                    .map_err(RuntimeError::DatabaseError)?;
                NDJSON::write_to_file(self.get_or_create_file(&msg)?, object!{
                    chatroom: chatroom,
                    message: message,
                })?;
            }
            current_message += 1;
            if current_message % 99 == 0 {
                pb.set_position(current_message);
            }
        }
        pb.finish();
        Ok(())
    }

    /// Create a file for the given chat, caching it so we don't need to build it later
    fn get_or_create_file(
        &mut self,
        _: &Message,
    ) -> Result<&mut BufWriter<File>, RuntimeError> {
        Ok(&mut self.file)
    }
}

impl<'a> Writer<'a, JsonValue> for NDJSON<'a> {
    fn format_message(&self, message: &Message, _: usize) -> Result<JsonValue, TableError> {
        // Data we want to write to a file
        let mut json_message = object!{};
        if let Some(service) = &message.service {
            json_message["service"] = from(service.as_str());
        }

        // Add message date
        json_message["timestamp"] = self.get_time(message);

        // Add message sender
        json_message["sender"] = from(self.config.who(
            message.handle_id,
            message.is_from_me(),
            &message.destination_caller_id,
        ));

        // If message was deleted, annotate it
        json_message["deleted"] = from(message.is_deleted());

        // Useful message metadata
        let message_parts = message.body();
        let mut attachments = Attachment::from_message(&self.config.db, message)?;
        let mut replies = message.get_replies(&self.config.db)?;

        // Index of where we are in the attachment Vector
        let mut attachment_index: usize = 0;

        // Render subject
        if let Some(subject) = &message.subject {
            json_message["subject"] = from(subject.as_str());
        }

        // Handle SharePlay
        json_message["shareplay"] = from(message.is_shareplay());

        // Handle Shared Location
        json_message["location"] = object!{
            started_sharing: message.started_sharing_location(),
            stopped_sharing: message.stopped_sharing_location(),
        };

        let mut json_body = array![];

        // Generate the message body from it's components
        for (idx, message_part) in message_parts.iter().enumerate() {
            let mut json_part = object!{};
            match message_part {
                // Fitness messages have a prefix that we need to replace with the opposite if who sent the message
                BubbleComponent::Text(text_attrs) => {
                    if let Some(text) = &message.text {
                        // Render edited message content, if applicable
                        if message.is_part_edited(idx) {
                            //
                            if let Some(edited_parts) = &message.edited_parts {
                                if let Some(edited) =
                                    self.format_edited(message, edited_parts, idx, "")
                                {
                                    json_part["body"] = edited;
                                    json_part["edited"] = from(true);
                                };
                            }
                            //
                        } else {
                            //
                            let mut text_body = array![];

                            for text_attr in text_attrs {
                                if let Some(message_content) =
                                    text.get(text_attr.start..text_attr.end)
                                {
                                    text_body.push(self.format_attributed(message_content, &text_attr.effect)).map_err(|e| TableError::Unknown(e.into()))?;
                                }
                            }

                            // If we failed to parse any text above, use the original text
                            if text_body.is_empty() {
                                text_body.push(from(text.as_str())).map_err(|e| TableError::Unknown(e.into()))?;
                            }

                            if text_body[0].to_string().starts_with(FITNESS_RECEIVER) {
                                text_body[0] = from(YOU);
                            }
                            //
                            json_part["body"] = text_body;
                        }
                    }
                }
                BubbleComponent::Attachment => match attachments.get_mut(attachment_index) {
                    Some(attachment) => {
                        if attachment.is_sticker {
                            json_part["sticker"] = self.format_sticker(attachment, message);
                        } else {
                            match self.format_attachment(attachment, message) {
                                Ok(result) => {
                                    attachment_index += 1;
                                    json_part["attachment"] = result;
                                }
                                Err(result) => {
                                    json_part["attachment_error"] = from(result);
                                }
                            }
                        }
                    }
                    // Attachment does not exist in attachments table
                    None => json_part["attachment_missing"] = from(true),
                },
                BubbleComponent::App => match self.format_app(message, &mut attachments, "") {
                    // We use an empty indent here because `format_app` handles building the entire message
                    Ok(ok_bubble) => json_part["app"] = ok_bubble,
                    Err(why) => json_part["app_error"] = from(why.to_string()),
                },
                BubbleComponent::Retracted => {
                    if let Some(edited_parts) = &message.edited_parts {
                        if let Some(edited) =
                            self.format_edited(message, edited_parts, idx, "")
                        {
                            json_part["retracted"] = edited;
                        };
                    }
                }
            };

            // Handle expressives
            if message.expressive_send_style_id.is_some() {
                json_part["expressive"] = self.format_expressive(message);
            }

            // Handle Reactions
            if let Some(reactions_map) = self.config.reactions.get(&message.guid) {
                if let Some(reactions) = reactions_map.get(&idx) {
                    let mut json_reactions = array![];
                    reactions
                        .iter()
                        .try_for_each(|reaction| -> Result<(), TableError> {
                            let formatted = self.format_reaction(reaction)?;
                            json_reactions.push(formatted).map_err(|e| TableError::Unknown(e.into()))?;
                            Ok(())
                        })?;

                    if !json_reactions.is_empty() {
                        json_part["reactions"] = json_reactions;
                    }
                }
            }

            // Handle Replies
            if let Some(replies) = replies.get_mut(&idx) {
                let mut replies_json = array![];
                replies
                    .iter_mut()
                    .try_for_each(|reply| -> Result<(), TableError> {
                        let _ = reply.generate_text(&self.config.db);
                        if !reply.is_reaction() {
                            replies_json.push(self.format_message(reply, 0)?).map_err(|e| TableError::Unknown(e.into()))?;
                        }
                        Ok(())
                    })?;
                json_part["replies"] = replies_json;
            }
            //end for
            json_body.push(json_part).map_err(|e| TableError::Unknown(e.into()))?;
        }

        json_message["body"] = json_body;
        // Add a note if the message is a reply
        if message.is_reply() {
            json_message["reply"] = from(true);
        }
        Ok(json_message)
    }

    fn format_attachment(
        &self,
        attachment: &'a mut Attachment,
        message: &Message,
    ) -> Result<JsonValue, &'a str> {
        self.config
            .options
            .attachment_manager
            .handle_attachment(message, attachment, self.config)
            .ok_or(attachment.filename())?;

        match attachment.as_bytes(&self.config.options.platform, &self.config.options.db_path, self.config.options.attachment_root.as_deref()) {
            Ok(data) => {
                Ok(object!{
                    attachment: BASE64_STANDARD.encode(&data.unwrap()),
                    kind: "attachment",
                })
            }
            Err(_) => {
                Err(attachment.filename())
            }
        }
    }

    fn format_sticker(&self, sticker: &'a mut Attachment, message: &Message) -> JsonValue {
        let who = self.config.who(
            message.handle_id,
            message.is_from_me(),
            &message.destination_caller_id,
        );
        match self.format_attachment(sticker, message) {
            Ok(path_to_sticker) => {
                let sticker_effect = sticker.get_sticker_effect(
                    &self.config.options.platform,
                    &self.config.options.db_path,
                    self.config.options.attachment_root.as_deref(),
                );
                if let Ok(Some(sticker_effect)) = sticker_effect {
                    return object!{
                        kind: "sticker",
                        message: "Sticker from {{ who }}: {{ path }}",
                        sticker: path_to_sticker,
                        effect: format!("{:?}", sticker_effect),
                    };
                }
                object!{
                    kind: "sticker",
                    message: "Sticker from {{ who }}: {{ path }}",
                    sticker: path_to_sticker,
                }
            }
            Err(path) => object!{
                kind: "sticker",
                error: true,
                who: who,
                path: path,
                message: "Sticker from {{ who }}: {{ path }}",
            },
        }
    }

    fn format_app(
        &self,
        message: &'a Message,
        attachments: &mut Vec<Attachment>,
        _: &str,
    ) -> Result<JsonValue, PlistParseError> {
        if let Variant::App(balloon) = message.variant() {
            let mut app_json = object!{
                kind: "app",
            };

            // Handwritten messages use a different payload type, so handle that first
            if matches!(balloon, CustomBalloon::Handwriting) {
                return Ok(self.format_handwriting(&HandwrittenMessage::new(), message));
            }

            if let Some(payload) = message.payload_data(&self.config.db) {
                // Handle URL messages separately since they are a special case
                if message.is_url() {
                    let parsed = parse_plist(&payload)?;
                    let bubble = URLMessage::get_url_message_override(&parsed)?;
                    app_json["url"] = match bubble {
                        URLOverride::Normal(balloon) => self.format_url(&balloon, message),
                        URLOverride::AppleMusic(balloon) => self.format_music(&balloon, message),
                        URLOverride::Collaboration(balloon) => {
                            self.format_collaboration(&balloon, message)
                        }
                        URLOverride::AppStore(balloon) => self.format_app_store(&balloon, message),
                        URLOverride::SharedPlacemark(balloon) => {
                            self.format_placemark(&balloon, message)
                        }
                    };
                // Handwriting uses a different payload type than the rest of the branches
                } else {
                    // Handle the app case
                    let parsed = parse_plist(&payload)?;
                    app_json["app"] = match AppMessage::from_map(&parsed) {
                        Ok(bubble) => match balloon {
                            CustomBalloon::Application(bundle_id) => {
                                self.format_generic_app(&bubble, bundle_id, attachments, message)
                            }
                            CustomBalloon::ApplePay => self.format_apple_pay(&bubble, message),
                            CustomBalloon::Fitness => self.format_fitness(&bubble, message),
                            CustomBalloon::Slideshow => self.format_slideshow(&bubble, message),
                            CustomBalloon::CheckIn => self.format_check_in(&bubble, message),
                            CustomBalloon::FindMy => self.format_find_my(&bubble, message),
                            CustomBalloon::Handwriting => unreachable!(),
                            CustomBalloon::URL => unreachable!(),
                        },
                        Err(why) => return Err(why),
                    };
                };
            } else {
                // Sometimes, URL messages are missing their payloads
                if message.is_url() {
                    if let Some(text) = &message.text {
                        return Ok(object!{
                            kind: "app",
                            text: text.to_string(),
                        });
                    }
                }
                return Err(PlistParseError::NoPayload);
            };
            Ok(app_json)
        } else {
            Err(PlistParseError::WrongMessageType)
        }
    }

    fn format_reaction(&self, msg: &Message) -> Result<JsonValue, TableError> {
        Ok(object!{
            kind: "reaction",
            reaction: match msg.variant() {
                Variant::Reaction(_, added, reaction) => {
                    object!{
                        kind: "reaction",
                        added: added,
                        reaction: from(format!("{:?}", reaction)),
                        by: self.config.who(msg.handle_id, msg.is_from_me(), &msg.destination_caller_id)
                    }
                }
                Variant::Sticker(_) => {
                    let mut paths = Attachment::from_message(&self.config.db, msg)?;
                    let mut json_sticker = object!{
                        kind: "sticker",
                        who: self.config.who(msg.handle_id, msg.is_from_me(), &msg.destination_caller_id),
                    };

                    // Sticker messages have only one attachment, the sticker image
                    if let Some(sticker) = paths.get_mut(0) {
                        json_sticker["sticker"] = self.format_sticker(sticker, msg);
                    } else {
                        json_sticker["error"] = from("Sticker from {{ who }} not found!");
                    }
                    json_sticker
                }
                _ => unreachable!(),
            }
        })
    }

    fn format_expressive(&self, msg: &'a Message) -> JsonValue {
        object!{
            kind: "expressive",
            expressive: match msg.get_expressive() {
                Expressive::Screen(effect) => object!{kind: "screen", screen: from(format!("{:?}", effect))},
                Expressive::Bubble(effect) => object!{kind: "bubble", bubble: from(format!("{:?}", effect))},
                Expressive::Unknown(effect) => object!{kind: "unknown", unknown: from(format!("{:?}", effect))},
                Expressive::None => object!{kind: "none"},
            },
        }
    }

    fn format_announcement(&self, msg: &'a Message) -> JsonValue {
        let mut who = self
            .config
            .who(msg.handle_id, msg.is_from_me(), &msg.destination_caller_id);
        // Rename yourself so we render the proper grammar here
        if who == ME {
            who = self.config.options.custom_name.as_deref().unwrap_or(YOU);
        }

        let timestamp = format(&msg.date(&self.config.offset));

        object!{
            kind: "announcement",
            timestamp: timestamp,
            who: who,
            announcement: match msg.get_announcement() {
                Some(announcement) => match announcement {
                    Announcement::NameChange(name) => object!{
                        kind: "name_change",
                        name: name,
                        message: "{{ timestamp }} {{ who }} renamed the conversation to {{ name }}"
                    },
                    Announcement::PhotoChange => object!{
                        kind: "photo_change",
                        message: "{{ timestamp }} {{ who }} changed the group photo."
                    },
                    Announcement::Unknown(num) => object!{
                        kind: "unknown",
                        action: *num,
                        message: "{{ timestamp }} {{ who }} performed unkown action {{ action }}."
                    },
                    Announcement::FullyUnsent => object!{
                        kind: "fully_unsent",
                        message: "{{ timestamp }} {{ who }} unsent a message!"
                    },
                },
                None => object!{
                    kind: "error",
                    error: "Unable to format announcement!",
                },
            },
        }
    }

    fn format_shareplay(&self) -> JsonValue {
        object!{
            kind: "shareplay",
            ended: true,
        }
    }

    fn format_shared_location(&self, msg: &'a Message) -> JsonValue {
        object!{
            kind: "shared_location",
            started: msg.started_sharing_location(),
            stopped: msg.stopped_sharing_location(),
        }
    }

    fn format_edited(
        &self,
        msg: &'a Message,
        edited_message: &'a EditedMessage,
        message_part_idx: usize,
        _: &str,
    ) -> Option<JsonValue> {
        if let Some(edited_message_part) = edited_message.part(message_part_idx) {
            let mut json_value = object!{kind: "edited"};
            let mut previous_timestamp: Option<&i64> = None;

            match edited_message_part.status {
                EditStatus::Edited => {
                    let mut edited = array![];
                    for event in &edited_message_part.edit_history {
                        edited.push(object!{
                            timestamp: match previous_timestamp {
                                // Original message get an absolute timestamp
                                None => {
                                    get_local_time(&event.date, &self.config.offset).map(|v| -> i64 {v.timestamp()}).ok()
                                }
                                // Subsequent edits get a relative timestamp
                                Some(prev_timestamp) => {
                                    let end = get_local_time(&event.date, &self.config.offset);
                                    let start = get_local_time(prev_timestamp, &self.config.offset);
                                    unreadable_diff(start, end)
                                }
                            },
                            text: from(event.text.as_str()),
                        }).ok();

                        // Update the previous timestamp for the next loop
                        previous_timestamp = Some(&event.date);
                    }
                    json_value["edited"] = edited;
                }
                EditStatus::Unsent => {
                    let mut unsent = object!{
                        who: if msg.is_from_me() {
                            self.config.options.custom_name.as_deref().unwrap_or(YOU)
                        } else {
                            "They"
                        },
                    };

                    match readable_diff(
                        msg.date(&self.config.offset),
                        msg.date_edited(&self.config.offset),
                    ) {
                        Some(diff) => {
                            unsent["diff"] = from(diff);
                            unsent["message"] = from("{{ who }} unsent this message part {{ diff }} after sending!");
                        }
                        None => {
                            unsent["message"] = from("{{ who }} unsent this message part!");
                        }
                    }


                    json_value["unsent"] = unsent;
                }
                EditStatus::Original => {
                    return None
                }
            }

            return Some(json_value);
        }
        None
    }

    fn format_attributed(&'a self, text: &'a str, attribute: &'a TextEffect) -> JsonValue {
       object!{
           kind: "attributed",
           text: match attribute {
               TextEffect::Default => from(object!{kind: "default", text: text}),
               TextEffect::Mention(mentioned) => self.format_mention(text, mentioned),
               TextEffect::Link(url) => self.format_link(text, url),
               TextEffect::OTP => self.format_otp(text),
               TextEffect::Styles(styles) => self.format_styles(text, styles),
               TextEffect::Animated(animation) => self.format_animated(text, animation),
               TextEffect::Conversion(unit) => self.format_conversion(text, unit),
           },
       }
    }

    fn write_to_file(file: &mut BufWriter<File>, text: JsonValue) -> Result<(), RuntimeError> {
        file.write_all((text.dump()+"\n").as_bytes())
            .map_err(RuntimeError::DiskError)
    }
}

impl<'a> BalloonFormatter<&'a Message, JsonValue> for NDJSON<'a> {
    fn format_url(&self, balloon: &URLMessage, _: &Message) -> JsonValue {
        object!{
            kind: "url",
            url: balloon.get_url().unwrap_or_else(|| ""),
            title: balloon.title.unwrap_or_else(|| ""),
            summary: balloon.summary.unwrap_or_else(|| ""),
        }
    }

    fn format_music(&self, balloon: &MusicMessage, _: &Message) -> JsonValue {
        object!{
            kind: "music",
            track_name: balloon.track_name.unwrap_or_else(|| ""),
            album: balloon.album.unwrap_or_else(|| ""),
            artist: balloon.artist.unwrap_or_else(|| ""),
            url: balloon.url.unwrap_or_else(|| ""),
        }
    }

    fn format_collaboration(&self, balloon: &CollaborationMessage, _: &Message) -> JsonValue {
        let mut json_value = object!{
            kind: "collaboration",
            title: balloon.title.unwrap_or_else(|| ""),
            url: balloon.url.unwrap_or_else(|| ""),
        };

        if let Some(name) = balloon.app_name {
            json_value["name"] = from(name);
        } else if let Some(bundle_id) = balloon.bundle_id {
            json_value["bundle_id"] = from(bundle_id);
        }
        json_value
    }

    fn format_app_store(&self, balloon: &AppStoreMessage, _: &'a Message) -> JsonValue {
        object!{
            kind: "app_store",
            name: balloon.app_name.unwrap_or_else(|| ""),
            description: balloon.description.unwrap_or_else(|| ""),
            platform: balloon.platform.unwrap_or_else(|| ""),
            genre: balloon.genre.unwrap_or_else(|| ""),
            url: balloon.url.unwrap_or_else(|| ""),
        }
    }

    fn format_placemark(&self, balloon: &PlacemarkMessage, _: &'a Message) -> JsonValue {
        object!{
            kind: "placemark",
            name: balloon.place_name.unwrap_or_else(|| ""),
            url: balloon.get_url().unwrap_or_else(|| ""),
            placemark_name: balloon.placemark.name.unwrap_or_else(|| ""),
            address: balloon.placemark.address.unwrap_or_else(|| ""),
            state: balloon.placemark.state.unwrap_or_else(|| ""),
            city: balloon.placemark.city.unwrap_or_else(|| ""),
            iso_country_code: balloon.placemark.iso_country_code.unwrap_or_else(|| ""),
            postal_code: balloon.placemark.postal_code.unwrap_or_else(|| ""),
            country: balloon.placemark.country.unwrap_or_else(|| ""),
            street: balloon.placemark.street.unwrap_or_else(|| ""),
            sub_administractive_area: balloon.placemark.sub_administrative_area.unwrap_or_else(|| ""),
            sub_locality: balloon.placemark.sub_locality.unwrap_or_else(|| ""),
        }
    }

    fn format_handwriting(&self, _: &HandwrittenMessage, _: &Message) -> JsonValue {
        object!{kind: "handwritten", supported: from(false)}
    }

    fn format_apple_pay(&self, balloon: &AppMessage, _: &Message) -> JsonValue {
        let mut json_body = object!{kind: "apply_pay"};
        if let Some(caption) = balloon.caption {
            json_body["caption"] = from(caption);
        }

        if let Some(ldtext) = balloon.ldtext {
            json_body["transaction"] = from(ldtext);
        } else {
            json_body["transaction"] = object!{unknown: true};
        }
        json_body
    }

    fn format_fitness(&self, balloon: &AppMessage, _: &Message) -> JsonValue {
        let mut json_value = object!{kind: "fitness"};
        if let Some(app_name) = balloon.app_name {
            json_value["app_name"] = from(app_name);
        }
        if let Some(ldtext) = balloon.ldtext {
            json_value["message"] = from(ldtext);
        } else {
            json_value["unknown"] = from(true);
        }
        json_value
    }

    fn format_slideshow(&self, balloon: &AppMessage, _: &Message) -> JsonValue {
        let mut json_value = object!{kind: "slideshow"};
        if let Some(ldtext) = balloon.ldtext {
            json_value["photo_album"] = from(ldtext);
        }

        if let Some(url) = balloon.url {
            json_value["url"] = from(url);
        }
        json_value
    }

    fn format_find_my(&self, balloon: &AppMessage, _: &'a Message) -> JsonValue {
        let mut json_value = object!{kind: "find_my"};
        if let Some(app_name) = balloon.app_name {
            json_value["name"] = from(app_name);
        }

        if let Some(ldtext) = balloon.ldtext {
            json_value["text"] = from(ldtext);
        }
        json_value
    }

    fn format_check_in(&self, balloon: &AppMessage, _: &'a Message) -> JsonValue {
        let mut json_value = object!{
            kind: "check_in",
            caption: balloon.caption.unwrap_or("Check In"),
        };

        let metadata: HashMap<&str, &str> = balloon.parse_query_string();

        // Before manual check-in
        if let Some(date_str) = metadata.get("estimatedEndTime") {
            // Parse the estimated end time from the message's query string
            let date_stamp = date_str.parse::<f64>().unwrap_or(0.) as i64 * TIMESTAMP_FACTOR;
            let date_time = get_local_time(&date_stamp, &0);
            let date_string = format(&date_time);
            json_value["expected_at"] = from(date_string);
        }
        // Expired check-in
        else if let Some(date_str) = metadata.get("triggerTime") {
            // Parse the estimated end time from the message's query string
            let date_stamp = date_str.parse::<f64>().unwrap_or(0.) as i64 * TIMESTAMP_FACTOR;
            let date_time = get_local_time(&date_stamp, &0);
            let date_string = format(&date_time);
            json_value["was_expected_at"] = from(date_string);
        }
        // Accepted check-in
        else if let Some(date_str) = metadata.get("sendDate") {
            // Parse the estimated end time from the message's query string
            let date_stamp = date_str.parse::<f64>().unwrap_or(0.) as i64 * TIMESTAMP_FACTOR;
            let date_time = get_local_time(&date_stamp, &0);
            let date_string = format(&date_time);
            json_value["checked_in_at"] = from(date_string);
        }
        json_value
    }

    fn format_generic_app(
        &self,
        balloon: &AppMessage,
        bundle_id: &str,
        attachments: &mut Vec<Attachment>,
        message: &Message,
    ) -> JsonValue {
        let mut json_value = object!{
            kind: "generic_app",
        };

        if let Some(name) = balloon.app_name {
            json_value["name"] = from(name);
        } else {
            json_value["bundle_id"] = from(bundle_id);
        }

        if let Some(url) = balloon.url {
            json_value["url"] = from(url);
        }

        if let Some(image) = balloon.image {
            json_value["image_url"] = from(image);
        } else if let Some(attachment) = attachments.get_mut(0) {
            if let Some(img) = self.format_attachment(attachment, message).ok() {
                json_value["image"] = from(img);
            }
        }

        if let Some(title) = balloon.title {
            json_value["title"] = from(title);
        }

        if let Some(subtitle) = balloon.subtitle {
            json_value["subtitle"] = from(subtitle);
        }

        if let Some(ldtext) = balloon.ldtext {
            json_value["ldtext"] = from(ldtext);
        }

        if let Some(caption) = balloon.caption {
            json_value["caption"] = from(caption);
        }

        if let Some(subcaption) = balloon.subcaption {
            json_value["subcaption"] = from(subcaption);
        }

        if let Some(trailing_caption) = balloon.trailing_caption {
            json_value["training_cation"] = from(trailing_caption);
        }

        if let Some(trailing_subcaption) = balloon.trailing_subcaption {
            json_value["training_subcation"] = from(trailing_subcaption);
        }

        json_value
    }
}

impl<'a> NDJSON<'a> {
    fn get_time(&self, message: &Message) -> JsonValue {
        let mut date_json = Object::new();
        date_json["timestamp"] = from(message.date(&self.config.offset).unwrap().timestamp());
        let read_after = message.time_until_read_unix(&self.config.offset);
        if let Some(time) = read_after {
            date_json["read_timestamp"] = object!{
                read_by_you: !message.is_from_me(),
                timestamp: time,
            };
        }
        from(date_json)
    }

    fn get_chatroom(&self, message: &Message) -> JsonValue {
        match self.config.conversation(message) {
            Some((chatroom, _)) => {
                object!{name: self.config.chatroom_name(chatroom)}
            }
            None => object!{orphaned: true},
        }
    }
}

impl<'a> TextEffectFormatter<JsonValue> for NDJSON<'a> {
    fn format_mention(&self, text: &str, mentioned: &str) -> JsonValue {
        object!{kind: "mention", mentioned: mentioned, text: text}
    }

    fn format_link(&self, text: &str, url: &str) -> JsonValue {
        object!{kind: "link", url: url, text: text}
    }

    fn format_otp(&self, text: &str) -> JsonValue {
        object!{kind: "otp", otp: text}
    }

    fn format_conversion(&self, text: &str, unit: &Unit) -> JsonValue {
        object!{kind: "conversion", text: text, unit: format!("{:?}", unit)}
    }

    fn format_styles(&self, text: &str, styles: &[Style]) -> JsonValue {
        object!{
            kind: "styles",
            text: text,
            styles: styles.iter().map(|e| format!("{:?}", e) ).collect::<Vec<String>>(),
        }
    }

    fn format_animated(&self, text: &str, animation: &Animation) -> JsonValue {
        object!{
            kind: "animated",
            text: text,
            animation: format!("{:?}", animation),
        }
    }
}