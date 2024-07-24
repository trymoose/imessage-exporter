/*!
 This module contains Data structures and models that represent message data.
*/

use crate::message_types::text_effects::TextEffect;

/// Defines the parts of a message bubble, i.e. the content that can exist in a single message.
#[derive(Debug, PartialEq, Eq)]
pub enum BubbleType<'a> {
    /// A text message with associated formatting
    Text(Vec<TextAttributes<'a>>),
    /// An attachment
    Attachment,
    /// An app integration
    App,
}

/// Defines different types of services we can receive messages from.
#[derive(Debug)]
pub enum Service<'a> {
    /// An iMessage
    #[allow(non_camel_case_types)]
    iMessage,
    /// A message sent as SMS
    SMS,
    /// Any other type of message
    Other(&'a str),
    /// Used when service field is not set
    Unknown,
}

/// Defines ranges and attributes parsed from [`typedstream`](crate::util::typedstream) `attributedBody` data.
/// 
/// For a given message, there will be ranges that specify the attributes associated with those ranges. For example:
/// 
/// Given message text with a [`Mention`](TextEffect::Mention) like:
/// 
/// `What's up, Christopher?`
/// 
/// There will be 3 ranges:
/// 
/// ```
/// use imessage_database::message_types::text_effects::TextEffect;
/// use imessage_database::tables::messages::models::{TextAttributes, BubbleType};
///  
/// let result = vec![BubbleType::Text(vec![
///     TextAttributes::new(0, 11, TextEffect::Default),  // `What's up, `
///     TextAttributes::new(11, 22, TextEffect::Mention), // `Christopher`
///     TextAttributes::new(22, 23, TextEffect::Default)  // `?`
/// ])];
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct TextAttributes<'a> {
    /// The start index of the affected range of message text
    pub start: usize,
    /// The end index of the affected range of message text
    pub end: usize,
    /// The effects applied to the specified range
    pub effect: TextEffect<'a>,
}

impl<'a> TextAttributes<'a> {
    pub fn new(start: usize, end: usize, effect: TextEffect<'a>) -> Self {
        Self { start, end, effect }
    }
}
