use crate::message_types::digital_touch::tap::DigitalTouchTap;
use crate::{
    error::digital_touch::DigitalTouchError,
    message_types::digital_touch::digital_touch_proto::{BaseMessage, TouchKind},
};
use protobuf::Message;
use crate::message_types::digital_touch::sketch::DigitalTouchSketch;

/// Parser for [digital touch](https://support.apple.com/guide/ipod-touch/send-a-digital-touch-effect-iph3fadba219/ios) iMessages.
///
/// This message type is not documented by Apple, but represents messages displayed as
/// `com.apple.DigitalTouchBalloonProvider`.
#[derive(Debug, PartialEq, Eq)]
pub enum DigitalTouchMessage {
    Tap(DigitalTouchTap),
    Sketch(DigitalTouchSketch),
}

#[derive(Debug, PartialEq, Clone, Eq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_bytes(buf: &[u8]) -> Color {
        Color::new(buf[0], buf[1], buf[2], buf[3])
    }

    pub fn tuple(&self) -> (u8, u8, u8, u8) {
        (self.r, self.g, self.b, self.a)
    }
}

impl DigitalTouchMessage {
    /// Converts a raw byte payload from the database into a [`DigitalTouchMessage`].
    pub fn from_payload(payload: &[u8]) -> Result<Self, DigitalTouchError> {
        let msg =
            BaseMessage::parse_from_bytes(payload).map_err(DigitalTouchError::ProtobufError)?;

        match msg.TouchKind.enum_value_or_default() {
            TouchKind::Unknown => Err(DigitalTouchError::UnknownDigitalTouchKind(msg.TouchKind.value())),
            TouchKind::Tap => DigitalTouchTap::from_payload(&msg),
            TouchKind::Drawing => DigitalTouchSketch::from_payload(&msg),
        }
    }
}

