use crate::{
    message_types::text_effects::{TextEffect, Unit},
    tables::messages::{
        models::{BubbleType, TextAttributes},
        Message,
    },
    util::typedstream::models::{Archivable, OutputData},
};

/// Character found in message body text that indicates attachment position
const ATTACHMENT_CHAR: char = '\u{FFFC}';
/// Character found in message body text that indicates app message position
const APP_CHAR: char = '\u{FFFD}';
/// A collection of characters that represent non-text content within body text
const REPLACEMENT_CHARS: [char; 2] = [ATTACHMENT_CHAR, APP_CHAR];

pub enum BubbleResult<'a> {
    New(BubbleType<'a>),
    Continuation(TextAttributes<'a>),
}

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

    // Create the output data
    let mut out_v = vec![];

    // Start to iterate over the ranges
    if let Some(components) = &message.components {
        // The first item is the text itself, so skip over it when iterating
        let mut idx = 1;
        let mut current_start;
        let mut current_end = 0;

        while idx < components.len() {
            // The first part of the range sometimes indicates the part number, but not always
            if let Some((_, length)) = get_range(components.get(idx)?) {
                current_start = current_end;
                current_end += *length as usize;
            } else {
                idx += 1;
                continue;
            }

            // The range is followed by a dictionary of attributes that map to that range
            idx += 1;
            let num_attrs = get_attribute_dict_length(components.get(idx));

            // The next set of values alternate between key and value pairs for the dictionary, if there are any
            if num_attrs > 0 {
                idx += 1;
            }

            // If there are no attributes, the default bubble will be applied
            // Otherwise, determine the bubble based on the attributes
            let slice: &[Archivable] = get_n_dict_objects(components, idx, num_attrs);

            // Determine the type of the bubble and add it to the body parts vec
            if let Some(bubble) = get_bubble_type(
                slice,
                message,
                current_start,
                current_end,
                &char_index_table,
            ) {
                match bubble {
                    BubbleResult::New(item) => out_v.push(item),
                    BubbleResult::Continuation(effect) => match out_v.last_mut() {
                        Some(BubbleType::Text(attrs)) => attrs.push(effect),
                        _ => out_v.push(BubbleType::Text(vec![effect])),
                    },
                }
            }

            // Advance the iterator by the number of attributes we just consumed
            idx += slice.len();
        }
    }
    (!out_v.is_empty()).then_some(out_v)
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
fn get_char_idx(text: &str, idx: usize, char_indices: &[usize]) -> usize {
    char_indices.get(idx).map_or(text.len(), |i| *i)
}

/// Get the number of key/value object pairs in a NSDictionary
fn get_attribute_dict_length(component: Option<&Archivable>) -> usize {
    if let Some(Archivable::Object(class, data)) = component {
        if class.name == "NSDictionary" {
            if let Some(OutputData::SignedInteger(length)) = data.first() {
                return (length * 2) as usize;
            }
        }
    }
    0
}

/// Get a specific number of objects that represent the content of a dictionary
fn get_n_dict_objects(components: &[Archivable], idx: usize, num_objects: usize) -> &[Archivable] {
    if num_objects == 0 {
        return &[];
    }
    let mut final_idx = idx + num_objects;
    for (idx, component) in components.iter().enumerate().skip(idx) {
        // Break the loop if we encounter a new range, which indicates we should move on to the next part
        if get_range(component).is_some() {
            break;
        }
        final_idx = idx;
    }
    components.get(idx..final_idx + 1).unwrap_or(&[])
}

/// Determine the type of bubble the current range represents
///
/// App messages are handled in [`Message::body()`]; they are detected by the presence of data in the `balloon_bundle_id` column.
fn get_bubble_type<'a>(
    components: &'a [Archivable],
    message: &'a Message,
    start: usize,
    end: usize,
    char_indices: &[usize],
) -> Option<BubbleResult<'a>> {
    for (idx, key) in components.iter().enumerate() {
        // In the future, we will detect TextEffects as well
        if let Archivable::Object(class, data) = key {
            if class.name == "NSString" {
                if let Some(OutputData::String(key_name)) = data.first().as_ref() {
                    let start = get_char_idx(message.text.as_ref()?, start, char_indices);
                    let end = get_char_idx(message.text.as_ref()?, end, char_indices);
                    match key_name.as_str() {
                        "__kIMFileTransferGUIDAttributeName" => {
                            return Some(BubbleResult::New(BubbleType::Attachment))
                        }
                        "__kIMMentionConfirmedMention" => {
                            return Some(BubbleResult::Continuation(TextAttributes::new(
                                start,
                                end,
                                TextEffect::Mention,
                            )));
                        }
                        "__kIMLinkAttributeName" => {
                            return Some(BubbleResult::Continuation(TextAttributes::new(
                                start,
                                end,
                                TextEffect::Link(get_link(components.get(idx + 2)?)?),
                            )));
                        }
                        "__kIMOneTimeCodeAttributeName" => {
                            return Some(BubbleResult::Continuation(TextAttributes::new(
                                start,
                                end,
                                TextEffect::OTP,
                            )));
                        }
                        "__kIMCalendarEventAttributeName" => {
                            return Some(BubbleResult::Continuation(TextAttributes::new(
                                start,
                                end,
                                TextEffect::Conversion(Unit::Timezone),
                            )));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Some(BubbleResult::Continuation(TextAttributes::new(
        get_char_idx(message.text.as_ref()?, start, char_indices),
        get_char_idx(message.text.as_ref()?, end, char_indices),
        TextEffect::Default,
    )))
}

fn get_link(component: &Archivable) -> Option<&str> {
    if let Archivable::Object(class, data) = component {
        if class.name == "NSString" {
            if let Some(OutputData::String(url)) = data.first().as_ref() {
                return Some(url);
            }
        }
    }
    None
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
                        out_v.push(BubbleType::Text(vec![TextAttributes::new(
                            start,
                            idx,
                            TextEffect::Default,
                        )]));
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
                out_v.push(BubbleType::Text(vec![TextAttributes::new(
                    start,
                    text.len(),
                    TextEffect::Default,
                )]));
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
        message_types::text_effects::{TextEffect, Unit},
        tables::messages::{
            body::parse_body_typedstream,
            models::{BubbleType, TextAttributes},
            Message,
        },
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
            vec![BubbleType::Text(vec![TextAttributes::new(
                0,
                10,
                TextEffect::Default
            )])]
        );
    }

    #[test]
    fn can_get_message_body_app() {
        let mut m = blank();
        m.text = Some("\u{FFFC}".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/AppMessage");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Attachment]
        );
    }

    #[test]
    fn can_get_message_body_simple_two() {
        let mut m = blank();
        m.text = Some("Test 3".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/AttributedBodyTextOnly2");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text(vec![TextAttributes::new(
                0,
                6,
                TextEffect::Default
            )])]
        );
    }

    #[test]
    fn can_get_message_body_multi_part() {
        let mut m = blank();
        m.text = Some("\u{FFFC}test 1\u{FFFC}test 2 \u{FFFC}test 3".to_string());

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
                BubbleType::Text(vec![TextAttributes::new(3, 9, TextEffect::Default)]),
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(12, 19, TextEffect::Default)]),
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(22, 28, TextEffect::Default)]),
            ]
        );
    }

    #[test]
    fn can_get_message_body_multi_part_deleted() {
        let mut m = blank();
        m.text = Some(
            "From arbitrary byte stream:\r\u{FFFC}To native Rust data structures:\r".to_string(),
        );

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/MultiPartWithDeleted");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![
                BubbleType::Text(vec![TextAttributes::new(0, 28, TextEffect::Default)]),
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(31, 63, TextEffect::Default)]),
            ]
        );
    }

    #[test]
    fn can_get_message_body_attachment() {
        let mut m = blank();
        m.text = Some(
            "\u{FFFC}This is how the notes look to me fyi, in case it helps make sense of anything"
                .to_string(),
        );

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/Attachment");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(3, 80, TextEffect::Default)]),
            ]
        );
    }

    #[test]
    fn can_get_message_body_attachment_i16() {
        let mut m = blank();
        m.text = Some("\u{FFFC}".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/AttachmentI16");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Attachment]
        );
    }

    #[test]
    fn can_get_message_body_url() {
        let mut m = blank();
        m.text = Some("https://twitter.com/xxxxxxxxx/status/0000223300009216128".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/URLMessage");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        m.components
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(idx, item)| println!("\t{idx}: {item:?}"));

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text(vec![TextAttributes::new(
                0,
                56,
                TextEffect::Link("https://twitter.com/xxxxxxxxx/status/0000223300009216128")
            )]),]
        );
    }

    #[test]
    fn can_get_message_body_mention() {
        let mut m = blank();
        m.text = Some("Test Dad ".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/Mention");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        m.components
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(idx, item)| println!("\t{idx}: {item:?}"));

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text(vec![
                TextAttributes::new(0, 5, TextEffect::Default),
                TextAttributes::new(5, 8, TextEffect::Mention),
                TextAttributes::new(8, 9, TextEffect::Default)
            ])]
        );
    }

    #[test]
    fn can_get_message_body_code() {
        let mut m = blank();
        m.text = Some("000123 is your security code. Don't share your code.".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/Code");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        m.components
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(idx, item)| println!("\t{idx}: {item:?}"));

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text(vec![
                TextAttributes::new(0, 6, TextEffect::OTP),
                TextAttributes::new(6, 52, TextEffect::Default),
            ])]
        );
    }

    #[test]
    fn can_get_message_body_phone() {
        let mut m = blank();
        m.text = Some("What about 0000000000".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/PhoneNumber");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        m.components
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(idx, item)| println!("\t{idx}: {item:?}"));

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text(vec![
                TextAttributes::new(0, 11, TextEffect::Default),
                TextAttributes::new(11, 21, TextEffect::Link("tel:0000000000")),
            ])]
        );
    }

    #[test]
    fn can_get_message_body_email() {
        let mut m = blank();
        m.text = Some("asdfghjklq@gmail.com might work".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/Email");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        m.components
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(idx, item)| println!("\t{idx}: {item:?}"));

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text(vec![
                TextAttributes::new(0, 20, TextEffect::Link("mailto:asdfghjklq@gmail.com")),
                TextAttributes::new(20, 31, TextEffect::Default),
            ])]
        );
    }

    #[test]
    fn can_get_message_body_date() {
        let mut m = blank();
        m.text = Some("Hi. Right now or tomorrow?".to_string());

        let typedstream_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/Date");
        let mut file = File::open(typedstream_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::from(&bytes);
        m.components = parser.parse().ok();

        m.components
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(idx, item)| println!("\t{idx}: {item:?}"));

        assert_eq!(
            parse_body_typedstream(&m).unwrap(),
            vec![BubbleType::Text(vec![
                TextAttributes::new(0, 17, TextEffect::Default),
                TextAttributes::new(17, 25, TextEffect::Conversion(Unit::Timezone)),
                TextAttributes::new(25, 26, TextEffect::Default),
            ])]
        );
    }
}

#[cfg(test)]
mod legacy_tests {
    use super::typedstream_tests::blank;

    use crate::{
        message_types::text_effects::TextEffect,
        tables::messages::{
            body::parse_body_legacy,
            models::{BubbleType, TextAttributes},
        },
    };

    #[test]
    fn can_get_message_body_single_emoji() {
        let mut m = blank();
        m.text = Some("🙈".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![BubbleType::Text(vec![TextAttributes::new(
                0,
                4,
                TextEffect::Default
            ),])]
        );
    }

    #[test]
    fn can_get_message_body_multiple_emoji() {
        let mut m = blank();
        m.text = Some("🙈🙈🙈".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![BubbleType::Text(vec![TextAttributes::new(
                0,
                12,
                TextEffect::Default
            ),])]
        );
    }

    #[test]
    fn can_get_message_body_text_only() {
        let mut m = blank();
        m.text = Some("Hello world".to_string());
        assert_eq!(
            parse_body_legacy(&m),
            vec![BubbleType::Text(vec![TextAttributes::new(
                0,
                11,
                TextEffect::Default
            ),])]
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
                BubbleType::Text(vec![TextAttributes::new(3, 14, TextEffect::Default),])
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
                BubbleType::Text(vec![TextAttributes::new(3, 14, TextEffect::Default),])
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
                BubbleType::Text(vec![TextAttributes::new(0, 3, TextEffect::Default),]),
                BubbleType::App,
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(9, 12, TextEffect::Default),]),
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(15, 20, TextEffect::Default),]),
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(23, 27, TextEffect::Default),]),
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
                BubbleType::Text(vec![TextAttributes::new(6, 9, TextEffect::Default),]),
                BubbleType::Attachment,
                BubbleType::Text(vec![TextAttributes::new(12, 17, TextEffect::Default),]),
                BubbleType::Attachment,
            ]
        );
    }
}
