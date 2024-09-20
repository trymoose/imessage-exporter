#![allow(private_bounds)]

use crate::message_types::digital_touch::fireball::DigitalTouchFireball;
use crate::message_types::digital_touch::heartbeat::DigitalTouchHeartbeat;
use crate::message_types::digital_touch::kiss::DigitalTouchKiss;
use crate::message_types::digital_touch::sketch::DigitalTouchSketch;
use crate::message_types::digital_touch::tap::DigitalTouchTap;
use crate::util::svg_canvas::SVGCanvas;
use crate::{
    error::digital_touch::DigitalTouchError,
    message_types::digital_touch::digital_touch_proto::{BaseMessage, TouchKind},
};
use protobuf::Message;
use std::collections::HashMap;

/// Parser for [digital touch](https://support.apple.com/guide/ipod-touch/send-a-digital-touch-effect-iph3fadba219/ios) iMessages.
///
/// This message type is not documented by Apple, but represents messages displayed as
/// `com.apple.DigitalTouchBalloonProvider`.
#[derive(Debug, PartialEq, Eq)]
pub enum DigitalTouchMessage {
    Tap(DigitalTouchTap),
    Sketch(DigitalTouchSketch),
    Kiss(DigitalTouchKiss),
    Heartbeat(DigitalTouchHeartbeat),
    Fireball(DigitalTouchFireball),
}

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub struct Point<T> {
    pub x: u16,
    pub y: u16,
    pub extra: T,
}

#[derive(Debug, Clone, PartialEq, Copy, Eq)]
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

    pub fn decode_all(buf: &[u8]) -> Vec<Color> {
        decode_bytes(buf, 4, Color::from_bytes)
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
            TouchKind::Sketch => DigitalTouchSketch::from_payload(&msg),
            TouchKind::Kiss => DigitalTouchKiss::from_payload(&msg),
            TouchKind::Heartbeat => DigitalTouchHeartbeat::from_payload(&msg),
            TouchKind::Fireball => DigitalTouchFireball::from_payload(&msg),
        }
    }
}

pub fn decode_bytes<T>(b: &[u8], count: usize, parse_fn: impl Fn(&[u8]) -> T) -> Vec<T> {
    let mut a = vec![];
    let mut idx = 0;
    while idx < b.len() {
        a.push(parse_fn(&b[idx..idx+count]));
        idx += count;
    }
    a
}

pub fn decode_u16s(buf: &[u8]) -> Vec<u16> {
    decode_bytes(buf, 2, |buf| u16::from_le_bytes([buf[0], buf[1]]))
}

pub fn decode_points<T: Copy>(raw_points: &[u8], extra: Vec<T>) -> Result<Vec<Point<T>>, DigitalTouchError> {
    let points = decode_bytes(raw_points, 4, |buf| (
        u16::from_le_bytes([buf[0], buf[1]]),
        u16::from_le_bytes([buf[2], buf[3]]),
    ));

    if points.len() != extra.len() {
        return Err(DigitalTouchError::ArraysDoNotMatch("points".to_string(), points.len(), "extra".to_string(), extra.len()));
    }

    Ok(extra.iter().zip(points).map(|(&extra, (x, y))| Point { x, y, extra }).collect())
}

pub trait SVGTapRender<T>: SVGTap<T> {
    fn render_svg(&self, canvas: &mut SVGCanvas) {
        let mut delay = 1;
        self.get_points().iter().for_each(|point| {
            let d = self.get_point_delay(point);
            delay += d;
            self.render_svg_tap(canvas, delay, point);
        });
    }
}

pub(crate) trait SVGTap<T> {
    fn render_svg_tap(&self, canvas: &mut SVGCanvas, delay: usize, point: &Point<T>) {
        let x =  canvas.fit_x(point.x as usize, u16::MAX as usize);
        let y =  canvas.fit_y(point.y as usize, u16::MAX as usize);
        canvas.write_elem(
            self.get_point_elem(),
            self.get_point_attrs(point, x, y),
            Some(vec![
                self.get_opacity_animation(delay),
            ].iter()
                .chain(self.get_other_animations(delay).iter())
                .map(|s| s.clone())
                .collect::<Vec<String>>()
                .join("\n")
            ),
        );
    }

    fn get_opacity_animation(&self, delay: usize) -> String {
        SVGCanvas::generate_elem("animate", HashMap::from([
            ("attributeName", "opacity".to_string()),
            ("values", "0.0; 1.0; 0.0".to_string()),
            ("keyTimes", "0; 0.25; 1".to_string()),
            ("dur", "1.5s".to_string()),
            ("begin", format!("{delay}ms")),
            ("repeatCount", "1".to_string()),
            ("restart", "whenNotActive".to_string()),
        ]), None)
    }

    fn get_other_animations(&self, delay: usize) -> Vec<String>;
    fn get_point_elem(&self) -> &str;
    fn get_point_attrs(&self, point: &Point<T>, x: usize, y: usize) -> HashMap<&str, String>;
    fn get_points(&self) -> &Vec<Point<T>>;
    fn get_point_delay(&self, p: &Point<T>) -> usize;
}