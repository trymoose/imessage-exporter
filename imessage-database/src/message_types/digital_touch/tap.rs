use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, TapMessage};
use crate::message_types::digital_touch::models::{decode_points, decode_u16s, Color, Point, SVGTap, SVGTapRender};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::util::svg_canvas::SVGCanvas;
use protobuf::Message;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalTouchTap {
    pub id: String,
    pub taps: Vec<Point<TapData>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TapData {
    pub color: Color,
    pub ms_delay: u16,
}

impl DigitalTouchTap {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = TapMessage::parse_from_bytes(base_message.TouchPayload.as_slice()).map_err(DigitalTouchError::ProtobufError)?;

        let colors = Color::decode_all(&msg.Color);
        let delays = decode_u16s(&msg.Delays);

        if colors.len() !=  delays.len() {
            return Err(DigitalTouchError::ArraysDoNotMatch("delays".to_string(), delays.len(), "colors".to_string(), colors.len()));
        }

        Ok(DigitalTouchMessage::Tap(DigitalTouchTap{
            id: base_message.ID.clone(),
            taps: merge_tap_data(&msg.Location, delays, colors)?,
        }))
    }
}

impl SVGTapRender<TapData> for DigitalTouchTap {}

impl SVGTap<TapData> for DigitalTouchTap {
    fn get_other_animations(&self, delay: usize) -> Vec<String> {
        vec![
            SVGCanvas::generate_elem("animate", HashMap::from([
                ("attributeName", "r".to_string()),
                ("from", "0%".to_string()),
                ("to", "50%".to_string()),
                ("dur", "1.5s".to_string()),
                ("begin", format!("{delay}ms")),
                ("repeatCount", "1".to_string()),
                ("restart", "whenNotActive".to_string()),
            ]), None),
        ]
    }

    fn get_point_elem(&self) -> &str {
        "circle"
    }

    fn get_point_attrs(&self, point: &Point<TapData>, x: usize, y: usize) -> HashMap<&str, String> {
        let (r, g, b, a) = point.extra.color.tuple();
        HashMap::from([
            ("cx", format!("{x}px")),
            ("cy", format!("{y}px")),
            ("fill", "none".to_string()),
            ("stroke-width", format!("{}", 30)),
            ("stroke", format!("rgba({r}, {g}, {b}, {a})")),
        ])
    }

    fn get_points(&self) -> &Vec<Point<TapData>> {
        &self.taps
    }

    fn get_point_delay(&self, p: &Point<TapData>) -> usize {
        p.extra.ms_delay as usize
    }
}


fn merge_tap_data(points: &Vec<u8>, delays: Vec<u16>, colors: Vec<Color>) -> Result<Vec<Point<TapData>>, DigitalTouchError> {
    decode_points(points, colors.iter().zip(delays).map(|(color, ms_delay)| {
        TapData{ color: color.clone(), ms_delay }
    }).collect())
}