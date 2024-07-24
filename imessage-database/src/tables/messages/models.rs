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

#[derive(Debug, PartialEq, Eq)]
pub struct TextAttributes<'a> {
    /// The start index of the affected range
    pub start: usize,
    /// The end index of the affected range
    pub end: usize,
    /// The effects applied to the specified range
    pub effect: TextEffect<'a>,
}

impl<'a> TextAttributes<'a> {
    pub fn new(start: usize, end: usize, effect: TextEffect<'a>) -> Self {
        Self { start, end, effect }
    }
}
