use crate::{
    message_types::text_effects::{Animation, Style, TextEffect},
    tables::messages::{models::BubbleType, Message},
};

/// Character found in message body text that indicates attachment position
const ATTACHMENT_CHAR: char = '\u{FFFC}';
/// Character found in message body text that indicates app message position
const APP_CHAR: char = '\u{FFFD}';
/// A collection of characters that represent non-text content within body text
const REPLACEMENT_CHARS: [char; 2] = [ATTACHMENT_CHAR, APP_CHAR];

/// Logic to use deserialized typedstream data to parse the message body
pub(crate) fn parse_body_typedstream(message: &Message) -> Vec<BubbleType> {
    todo!()
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
mod tests {
    use crate::{
        message_types::text_effects::TextEffect,
        tables::messages::{models::BubbleType, Message},
    };

    fn blank() -> Message {
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
    fn can_get_message_body_single_emoji() {
        let mut m = blank();
        m.text = Some("🙈".to_string());
        assert_eq!(m.body(), vec![BubbleType::Text("🙈", TextEffect::Default)]);
    }

    #[test]
    fn can_get_message_body_multiple_emoji() {
        let mut m = blank();
        m.text = Some("🙈🙈🙈".to_string());
        assert_eq!(
            m.body(),
            vec![BubbleType::Text("🙈🙈🙈", TextEffect::Default)]
        );
    }

    #[test]
    fn can_get_message_body_text_only() {
        let mut m = blank();
        m.text = Some("Hello world".to_string());
        assert_eq!(
            m.body(),
            vec![BubbleType::Text("Hello world", TextEffect::Default)]
        );
    }

    #[test]
    fn can_get_message_body_attachment_text() {
        let mut m = blank();
        m.text = Some("\u{FFFC}Hello world".to_string());
        assert_eq!(
            m.body(),
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
            m.body(),
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
            m.body(),
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
            m.body(),
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
