/*!
 Logic and containers for the `message_summary_info` of an edited or unsent iMessage.
*/

use plist::Value;

use crate::{
    error::plist::PlistParseError,
    message_types::variants::BalloonProvider,
    util::{
        dates::TIMESTAMP_FACTOR,
        plist::{extract_array_key, extract_bytes_key, extract_dictionary, extract_int_key},
        streamtyped::parse,
        typedstream::parser::TypedStreamReader,
    },
};

#[derive(Debug, PartialEq, Eq)]
pub enum EditStatus {
    Edited,
    Unsent,
    Original,
}

#[derive(Debug, PartialEq, Eq)]
pub struct EditedEvent<'a> {
    /// The date the message part was edited
    pub date: i64,
    /// The content of the edited message part in [`streamtyped`](crate::util::streamtyped) format
    pub text: String,
    /// A GUID reference to another message
    pub guid: Option<&'a str>,
}

impl<'a> EditedEvent<'a> {
    fn new(date: i64, text: String, guid: Option<&'a str>) -> Self {
        Self { date, text, guid }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EditedMessagePart<'a> {
    pub status: EditStatus,
    pub edit_history: Vec<EditedEvent<'a>>,
}

impl<'a> Default for EditedMessagePart<'a> {
    fn default() -> Self {
        Self {
            status: EditStatus::Original,
            edit_history: vec![],
        }
    }
}

/// iMessage permits editing sent messages up to five times
/// within 15 minutes of sending the first message and unsending
/// sent messages within 2 minutes.
///
/// Edited or unsent messages are stored with a `NULL` `text` field.
/// Edited messages include `message_summary_info` that contains an array of
/// [`typedstream`](crate::util::typedstream) data where each array item contains the edited
/// message. The order in the array represents the order the messages
/// were edited in, i.e. item 0 was the original and the last item is
/// the current message.
///
/// For each dictionary item in this array, The `d` key represents the
/// time the message was edited and the `t` key represents the message's
/// `attributedBody` text in the [`typedstream`](crate::util::typedstream) format.
///
/// There is no data in the array if the message was unsent.
///
/// Apple describes editing and unsending messages [here](https://support.apple.com/guide/iphone/unsend-and-edit-messages-iphe67195653/ios).
#[derive(Debug, PartialEq, Eq)]
pub struct EditedMessage<'a> {
    pub parts: Vec<EditedMessagePart<'a>>,
}

impl<'a> BalloonProvider<'a> for EditedMessage<'a> {
    fn from_map(payload: &'a Value) -> Result<Self, PlistParseError> {
        // Parse payload
        let plist_root = payload.as_dictionary().ok_or_else(|| {
            PlistParseError::InvalidType("root".to_string(), "dictionary".to_string())
        })?;

        // Get the parts of the message that may have been altered
        let message_parts = extract_dictionary(plist_root, "otr")?;

        // Prefill edited data
        let mut edited = Self::with_capacity(message_parts.len());
        message_parts
            .values()
            .for_each(|_| edited.parts.push(EditedMessagePart::default()));

        let maybe_edited_message_events = extract_dictionary(plist_root, "ec").ok();
        let maybe_unsent_message_indexes = extract_array_key(plist_root, "rp").ok();

        if let Some(edited_message_events) = maybe_edited_message_events {
            for (idx, (key, events)) in edited_message_events.iter().enumerate() {
                let events = events
                    .as_array()
                    .ok_or_else(|| PlistParseError::InvalidTypeIndex(idx, "array".to_string()))?;
                let parsed_key = key.parse::<usize>().unwrap();

                for event in events {
                    let message_data = event.as_dictionary().ok_or_else(|| {
                        PlistParseError::InvalidTypeIndex(idx, "dictionary".to_string())
                    })?;

                    let timestamp = extract_int_key(message_data, "d")? * TIMESTAMP_FACTOR;

                    let typedstream = extract_bytes_key(message_data, "t")?;

                    let mut parser = TypedStreamReader::from(typedstream);
                    let text = match parser
                        .parse()
                        .ok()
                        .as_ref()
                        .and_then(|items| items.first())
                        .and_then(|item| item.deserialize_as_nsstring())
                        .map(String::from)
                    {
                        Some(text) => text,
                        None => {
                            eprintln!("fallback!");
                            parse(typedstream.to_vec())
                                .map_err(PlistParseError::StreamTypedError)?
                        }
                    };

                    let guid = message_data.get("bcg").and_then(|item| item.as_string());

                    if let Some(item) = edited.parts.get_mut(parsed_key) {
                        item.status = EditStatus::Edited;
                        item.edit_history
                            .push(EditedEvent::new(timestamp, text, guid))
                    }
                }
            }
        }

        if let Some(unsent_message_indexes) = maybe_unsent_message_indexes {
            for (idx, unsent_message_idx) in unsent_message_indexes.iter().enumerate() {
                let parsed_idx = unsent_message_idx
                    .as_signed_integer()
                    .ok_or_else(|| PlistParseError::InvalidTypeIndex(idx, "int".to_string()))?
                    as usize;
                if let Some(item) = edited.parts.get_mut(parsed_idx) {
                    item.status = EditStatus::Unsent;
                }
            }
        }

        Ok(edited)
    }
}

impl<'a> EditedMessage<'a> {
    /// A new message with a preallocated capacity
    fn with_capacity(capacity: usize) -> Self {
        EditedMessage {
            parts: Vec::with_capacity(capacity),
        }
    }

    /// Gets the edited message data for the given message part index
    pub fn part(&self, index: usize) -> Option<&EditedMessagePart> {
        self.parts.get(index)
    }

    /// Gets the number of parts that were edited
    pub fn items(&self) -> usize {
        self.parts.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::message_types::edited::{EditStatus, EditedEvent, EditedMessagePart};
    use crate::message_types::{edited::EditedMessage, variants::BalloonProvider};
    use plist::Value;
    use std::env::current_dir;
    use std::fs::File;

    #[test]
    fn test_parse_edited() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/edited_message/Edited.plist");
        let plist_data = File::open(plist_path).unwrap();
        let plist = Value::from_reader(plist_data).unwrap();
        let parsed = EditedMessage::from_map(&plist).unwrap();

        let expected = EditedMessage {
            parts: vec![EditedMessagePart {
                status: EditStatus::Edited,
                edit_history: vec![
                    EditedEvent::new(690513474000000000, "First message  ".to_string(), None),
                    EditedEvent::new(690513480000000000, "Edit 1".to_string(), None),
                    EditedEvent::new(690513485000000000, "Edit 2".to_string(), None),
                    EditedEvent::new(690513494000000000, "Edited message".to_string(), None),
                ],
            }],
        };

        assert_eq!(parsed, expected);

        let expected_item = Some(expected.parts.first().unwrap());
        assert_eq!(parsed.part(0), expected_item);
    }

    #[test]
    fn test_parse_edited_to_link() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/edited_message/EditedToLink.plist");
        let plist_data = File::open(plist_path).unwrap();
        let plist = Value::from_reader(plist_data).unwrap();
        let parsed = EditedMessage::from_map(&plist).unwrap();

        let expected = EditedMessage {
            parts: vec![
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![], // The first part of this is the URL preview
                },
                EditedMessagePart {
                    status: EditStatus::Edited,
                    edit_history: vec![
                        EditedEvent::new(690514004000000000, "here we go!".to_string(), None),
                        EditedEvent::new(
                            690514772000000000,
                            "https://github.com/ReagentX/imessage-exporter/issues/10".to_string(),
                            Some("292BF9C6-C9B8-4827-BE65-6EA1C9B5B384"),
                        ),
                    ],
                },
            ],
        };

        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_edited_to_link_and_back() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/edited_message/EditedToLinkAndBack.plist");
        let plist_data = File::open(plist_path).unwrap();
        let plist = Value::from_reader(plist_data).unwrap();
        let parsed = EditedMessage::from_map(&plist).unwrap();

        let expected = EditedMessage {
            parts: vec![EditedMessagePart {
                status: EditStatus::Edited,
                edit_history: vec![
                    EditedEvent::new(
                        690514809000000000,
                        "This is a normal message".to_string(),
                        None,
                    ),
                    EditedEvent::new(
                        690514819000000000,
                        "Edit to a url https://github.com/ReagentX/imessage-exporter/issues/10"
                            .to_string(),
                        Some("0B9103FE-280C-4BD0-A66F-4EDEE3443247"),
                    ),
                    EditedEvent::new(
                        690514834000000000,
                        "And edit it back to a normal message...".to_string(),
                        Some("0D93DF88-05BA-4418-9B20-79918ADD9923"),
                    ),
                ],
            }],
        };

        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_deleted() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/edited_message/Deleted.plist");
        let plist_data = File::open(plist_path).unwrap();
        let plist = Value::from_reader(plist_data).unwrap();
        let parsed = EditedMessage::from_map(&plist).unwrap();

        let expected = EditedMessage {
            parts: vec![EditedMessagePart {
                status: EditStatus::Unsent,
                edit_history: vec![],
            }],
        };

        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_multipart_deleted() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/edited_message/MutliPartOneDeleted.plist");
        let plist_data = File::open(plist_path).unwrap();
        let plist = Value::from_reader(plist_data).unwrap();
        let parsed = EditedMessage::from_map(&plist).unwrap();

        let expected = EditedMessage {
            parts: vec![
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![],
                },
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![],
                },
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![],
                },
                EditedMessagePart {
                    status: EditStatus::Unsent,
                    edit_history: vec![],
                },
            ],
        };

        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_multipart_edited_and_deleted() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/edited_message/EditedAndDeleted.plist");
        let plist_data = File::open(plist_path).unwrap();
        let plist = Value::from_reader(plist_data).unwrap();
        let parsed = EditedMessage::from_map(&plist).unwrap();

        let expected = EditedMessage {
            parts: vec![
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![],
                },
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![],
                },
                EditedMessagePart {
                    status: EditStatus::Edited,
                    edit_history: vec![
                        EditedEvent::new(743907180000000000, "Second message".to_string(), None),
                        EditedEvent::new(
                            743907190000000000,
                            "Second message got edited!".to_string(),
                            None,
                        ),
                    ],
                },
            ],
        };

        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_multipart_edited_and_unsent() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/edited_message/EditedAndUnsent.plist");
        let plist_data = File::open(plist_path).unwrap();
        let plist = Value::from_reader(plist_data).unwrap();
        let parsed = EditedMessage::from_map(&plist).unwrap();

        let expected = EditedMessage {
            parts: vec![
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![],
                },
                EditedMessagePart {
                    status: EditStatus::Original,
                    edit_history: vec![],
                },
                EditedMessagePart {
                    status: EditStatus::Edited,
                    edit_history: vec![
                        EditedEvent::new(743907435000000000, "Second test".to_string(), None),
                        EditedEvent::new(
                            743907448000000000,
                            "Second test was edited!".to_string(),
                            None,
                        ),
                    ],
                },
                EditedMessagePart {
                    status: EditStatus::Unsent,
                    edit_history: vec![],
                },
            ],
        };

        assert_eq!(parsed, expected);
    }
}
