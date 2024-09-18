use std::collections::HashMap;
use protobuf::Message;
use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, TapMessage};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::message_types::digital_touch::models::{decode_bytes, Color, Point};
use crate::util::svg_canvas::SVGCanvas;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalTouchTap {
    pub id: String,
    pub taps: Vec<TapPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapPoint {
    pub point: Point,
    pub color: Color,
    pub ms_delay: u16,
}

impl DigitalTouchTap {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = TapMessage::parse_from_bytes(base_message.TouchPayload.as_slice()).map_err(DigitalTouchError::ProtobufError)?;

        let colors = decode_color_buf(&msg.Color);
        let points = decode_point_buf(&msg.Location);
        let delays = decode_delay_buf(&msg.Delays);

        if colors.len() != points.len() || points.len() !=  delays.len() {
            return Err(DigitalTouchError::TapArraysDoNotMatch(delays.len(), points.len(), colors.len()));
        }

        Ok(DigitalTouchMessage::Tap(DigitalTouchTap{
            id: base_message.ID.clone(),
            taps: merge_tap_data(points, delays, colors),
        }))
    }

    pub fn render_svg(&self, canvas: &mut SVGCanvas) {
        let mut delay = 1;
        self.taps.iter().for_each(|tap| {
            delay += tap.ms_delay;
            render_svg_tap(canvas, delay, tap);
        });
    }
}

fn render_svg_tap(canvas: &mut SVGCanvas, delay: u16, tap: &TapPoint) {
    let (r, g, b, a) = tap.color.tuple();

    let x =  canvas.fit_x(tap.point.x as usize, u16::MAX as usize);
    let y =  canvas.fit_y(tap.point.y as usize, u16::MAX as usize);

    canvas.write_elem("circle", HashMap::from([
        ("cx", format!("{x}px")),
        ("cy", format!("{y}px")),
        ("fill", "none".to_string()),
        ("stroke-width", format!("{}", 30)),
        ("stroke", format!("rgba({r}, {g}, {b}, {a})")),
    ]), Some(vec![
        SVGCanvas::generate_elem("animate", HashMap::from([
            ("attributeName", "r".to_string()),
            ("from", "0%".to_string()),
            ("to", "50%".to_string()),
            ("dur", "1.5s".to_string()),
            ("begin", format!("{delay}ms")),
            ("repeatCount", "1".to_string()),
            ("restart", "whenNotActive".to_string()),
        ]), None),
        SVGCanvas::generate_elem("animate", HashMap::from([
            ("attributeName", "opacity".to_string()),
            ("values", "0.0; 1.0; 0.0".to_string()),
            ("keyTimes", "0; 0.25; 1".to_string()),
            ("dur", "1.5s".to_string()),
            ("begin", format!("{delay}ms")),
            ("repeatCount", "1".to_string()),
            ("restart", "whenNotActive".to_string()),
        ]), None),
    ].join("\n")));
}

fn decode_color_buf(buf: &[u8]) -> Vec<Color> {
    decode_bytes(buf, 4, Color::from_bytes)
}

fn decode_point_buf(buf: &[u8]) -> Vec<(u16, u16)> {
    decode_bytes(buf, 4, |buf| (
        u16::from_le_bytes([buf[0], buf[1]]),
        u16::from_le_bytes([buf[2], buf[3]]),
    ))
}

fn decode_delay_buf(buf: &[u8]) -> Vec<u16> {
    decode_bytes(buf, 2, |buf| u16::from_le_bytes([buf[0], buf[1]]))
}

fn merge_tap_data(points: Vec<(u16, u16)>, delays: Vec<u16>, colors: Vec<Color>) -> Vec<TapPoint> {
    colors.iter().zip(delays).zip(points).map(|((color, delay), (x, y))| TapPoint{
        point: Point { x, y },
        color: color.clone(),
        ms_delay: delay,
    }).collect()
}