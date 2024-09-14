use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::handwriting::HandwrittenMessage;

/// Parser for [digital touch](https://support.apple.com/guide/ipod-touch/send-a-digital-touch-effect-iph3fadba219/ios) iMessages.
///
/// This message type is not documented by Apple, but represents messages displayed as
/// `com.apple.DigitalTouchBalloonProvider`.
#[derive(Debug, PartialEq, Eq)]
pub struct DigitalTouchMessage {
}

impl DigitalTouchMessage {
    /// Converts a raw byte payload from the database into a [`DigitalTouchMessage`].
    pub fn from_payload(payload: &[u8]) -> Result<Self, DigitalTouchError> {
        todo!()
    }
}