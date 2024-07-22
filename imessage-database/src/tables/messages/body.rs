use core::num;

use crate::{
    message_types::text_effects::{Animation, Style, TextEffect},
    tables::messages::{models::BubbleType, Message},
    util::typedstream::models::{Archivable, OutputData},
};

/// Character found in message body text that indicates attachment position
const ATTACHMENT_CHAR: char = '\u{FFFC}';
/// Character found in message body text that indicates app message position
const APP_CHAR: char = '\u{FFFD}';
/// A collection of characters that represent non-text content within body text
const REPLACEMENT_CHARS: [char; 2] = [ATTACHMENT_CHAR, APP_CHAR];

/// Logic to use deserialized typedstream data to parse the message body
pub(crate) fn parse_body_typedstream(message: &Message) -> Option<Vec<BubbleType>> {
    // If there is no parsed body text, escape early
    message.text.as_ref()?;

    // We want to index into the message text, so we need a table to align
    // Apple's indexes with the actual chars, not the bytes
    let char_index_table: Vec<usize> = message
        .text
        .as_ref()?
        .char_indices()
        .map(|(a, _)| a)
        .collect();
    println!("{char_index_table:?}");

    // Create the output data
    let mut out_v = vec![];

    // Start to iterate over the ranges
    if let Some(components) = &message.components {
        // The first item is the text itself, so skip over it when iterating
        let mut idx = 1;
        let mut current_start = 0;
        let mut current_end = 0;

        while idx < components.len() {
            if let Some((item, length)) = get_range(components.get(idx)?) {
                println!("got length {:?}", length);
                if *item > 1 {
                    current_start = current_end;
                }
                current_end += *length as usize;
            }

            println!("range: {current_start}..{current_end}");

            // The range is followed by a dictionary of attributes that map to that range
            idx += 1;
            let num_attrs = get_attribute_dict_length(components.get(idx)?)?;
            println!("Dict length: {num_attrs:?}");

            // The next set of values alternate between key and value pairs for the dictionary
            idx += 1;

            let slice = &components[idx..idx + num_attrs];
            let bubble = get_bubble_type(
                slice,
                message,
                current_start,
                current_end,
                &char_index_table,
            )?;

            out_v.push(bubble);

            println!("{:?}", out_v);

            idx += num_attrs;
        }
    }
    if !out_v.is_empty() {
        return Some(out_v);
    }
    None
}

fn get_range(component: &Archivable) -> Option<(&i64, &u64)> {
    if let Archivable::Data(items) = component {
        if items.len() == 2 {
            if let (OutputData::SignedInteger(item), OutputData::UnsignedInteger(end)) =
                (items.first()?, items.get(1)?)
            {
                return Some((item, end));
            }
        }
    }
    None
}

/// Given the attributedBody range indexes, get the substring from the Rust representations `char_indices()`
fn get_substring<'a>(text: &'a str, start: usize, end: usize, char_indices: &[usize]) -> &'a str {
    let start_byte = char_indices.get(start).map_or(text.len(), |i| *i);
    let end_byte = char_indices.get(end).map_or(text.len(), |i| *i);
    text[start_byte..end_byte].trim()
}

/// Get the number of key/value pairs in a NSDictionary
fn get_attribute_dict_length(component: &Archivable) -> Option<usize> {
    if let Archivable::Object(class, data) = component {
        if class.name == "NSDictionary" {
            if let Some(OutputData::SignedInteger(length)) = data.first() {
                return Some((length * 2) as usize);
            }
        }
    }
    None
}

/// Determine the type of bubble the current range represents
fn get_bubble_type<'a>(
    components: &'a [Archivable],
    message: &'a Message,
    start: usize,
    end: usize,
    char_indices: &[usize],
) -> Option<BubbleType<'a>> {
    let keys = components.iter().step_by(2);
    for key in keys {
        println!("{key:?}");
        // If the dict contains key "__kIMFileTransferGUIDAttributeName" it is an attachment
        // If the dict contains key "__kIMMentionConfirmedMention" it is a mention
        // In the future, we will detect TextEffects as well
        if let Archivable::Object(class, data) = key {
            if class.name == "NSString" {
                if let Some(OutputData::String(key_name)) = data.first().as_ref() {
                    match key_name.as_str() {
                        "__kIMFileTransferGUIDAttributeName" => return Some(BubbleType::Attachment),
                        "__kIMMentionConfirmedMention" => {
                            return Some(BubbleType::Text(
                                get_substring(message.text.as_ref()?, start, end, char_indices),
                                TextEffect::Mention,
                            ))
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Some(BubbleType::Text(
        get_substring(message.text.as_ref()?, start, end, char_indices),
        TextEffect::Default,
    ))
}

/// Fallback logic to parse the body from the message string content
pub(crate) fn parse_body_legacy(message: &Message) -> Vec<BubbleType> {
    let mut out_v = vec![];
    // Naive logic for when `typedstream` component parsing fails
    match &message.text {
        Some(text) => {
            let mut start: usize = 0;
            let mut end: usize = 0;

            for (idx, char) in text.char_indices() {
                if REPLACEMENT_CHARS.contains(&char) {
                    if start < end {
                        out_v.push(BubbleType::Text(
                            text[start..idx].trim(),
                            TextEffect::Default,
                        ));
                    }
                    start = idx + 1;
                    end = idx;
                    match char {
                        ATTACHMENT_CHAR => out_v.push(BubbleType::Attachment),
                        APP_CHAR => out_v.push(BubbleType::App),
                        _ => {}
                    };
                } else {
                    if start > end {
                        start = idx;
                    }
                    end = idx;
                }
            }
            if start <= end && start < text.len() {
                out_v.push(BubbleType::Text(text[start..].trim(), TextEffect::Default));
            }
            out_v
        }
        None => out_v,
    }
}

#[cfg(test)]
mod typedstream_tests {
    use std::{env::current_dir, fs::File, io::Read};

    use crate::{
        message_types::text_effects::TextEffect,
        tables::messages::{body::parse_body_typedstream, models::BubbleType, Message},
        util::typedstream::parser::TypedStreamReader,
    };

    pub(super) fn blank() -> Message {
        Message {
            rowid: i32::default(),
            guid: String::default(),
            text: None,
            service: Some("iMessage".to_string()),
            handle_id: Some(i32::default()),
            destination_caller_id: None,
            subject: None,
            date: i64::default(),
            date_read: i64::default(),
            date_delivered: i64::default(),
            is_from_me: false,
            is_read: false,
            item_type: 0,
            other_handle: 0,
            share_status: false,
            share_direction: false,
            group_title: None,
            group_action_type: 0,
            associated_message_guid: None,
            associated_message_type: Some(i32::default()),
            balloon_bundle_id: None,
            expressive_send_style_id: None,
            thread_originator_guid: None,
            thread_originator_part: None,
            date_edited: 0,
            chat_id: None,
            num_attachments: 0,
            deleted_from: None,
            num_replies: 0,
            components: None,
        }
    }

    #[test]
    fn can_get_message_body_simple() {
        let mut m = blank();
        m.text = Some("Noter test".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/AttributedBodyTextOnly");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text("Noter test", TextEffect::Default)]
        );
    }

    #[test]
    fn can_get_message_body_multi_part() {
        let mut m = blank();
        m.text = Some("￼test 1￼test 2 ￼test 3".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/Multipart");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![
                BubbleType::Attachment,
                BubbleType::Text("test 1", TextEffect::Default),
                BubbleType::Attachment,
                BubbleType::Text("test 2", TextEffect::Default),
                BubbleType::Attachment,
                BubbleType::Text("test 3", TextEffect::Default)
            ]
        );
    }
}

#[cfg(test)]
mod legacy_tests {
    use super::typedstream_tests::blank;

    use crate::{
        message_types::text_effects::TextEffect,
        tables::messages::{body::parse_body_legacy, models::BubbleType},
    };

    #[test]
    fn can_get_message_body_single_emoji() {
        let mut m = blank();
        m.text = Some("🙈".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![BubbleType::Text("🙈", TextEffect::Default)]
        );
    }

    #[test]
    fn can_get_message_body_multiple_emoji() {
        let mut m = blank();
        m.text = Some("🙈🙈🙈".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![BubbleType::Text("🙈🙈🙈", TextEffect::Default)]
        );
    }

    #[test]
    fn can_get_message_body_text_only() {
        let mut m = blank();
        m.text = Some("Hello world".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![BubbleType::Text("Hello world", TextEffect::Default)]
        );
    }

    #[test]
    fn can_get_message_body_attachment_text() {
        let mut m = blank();
        m.text = Some("\u{FFFC}Hello world".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![
                BubbleType::Attachment,
                BubbleType::Text("Hello world", TextEffect::Default)
            ]
        );
    }

    #[test]
    fn can_get_message_body_app_text() {
        let mut m = blank();
        m.text = Some("\u{FFFD}Hello world".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![
                BubbleType::App,
                BubbleType::Text("Hello world", TextEffect::Default)
            ]
        );
    }

    #[test]
    fn can_get_message_body_app_attachment_text_mixed_start_text() {
        let mut m = blank();
        m.text = Some("One\u{FFFD}\u{FFFC}Two\u{FFFC}Three\u{FFFC}four".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![
                BubbleType::Text("One", TextEffect::Default),
                BubbleType::App,
                BubbleType::Attachment,
                BubbleType::Text("Two", TextEffect::Default),
                BubbleType::Attachment,
                BubbleType::Text("Three", TextEffect::Default),
                BubbleType::Attachment,
                BubbleType::Text("four", TextEffect::Default)
            ]
        );
    }

    #[test]
    fn can_get_message_body_app_attachment_text_mixed_start_app() {
        let mut m = blank();
        m.text = Some("\u{FFFD}\u{FFFC}Two\u{FFFC}Three\u{FFFC}".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![
                BubbleType::App,
                BubbleType::Attachment,
                BubbleType::Text("Two", TextEffect::Default),
                BubbleType::Attachment,
                BubbleType::Text("Three", TextEffect::Default),
                BubbleType::Attachment
            ]
        );
    }
}
