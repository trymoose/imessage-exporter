use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, KissMessage};
use crate::message_types::digital_touch::models::{decode_points, decode_u16s, Point, SVGTap, SVGTapRender};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::util::svg_canvas::SVGCanvas;
use protobuf::Message;
use std::collections::HashMap;
use std::f64::consts::PI;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalTouchKiss {
    pub id: String,
    pub kisses: Vec<Point<KissData>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KissData {
    pub ms_delay: u16,
    pub rads: u16,
}

impl DigitalTouchKiss {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = KissMessage::parse_from_bytes(base_message.TouchPayload.as_slice()).map_err(DigitalTouchError::ProtobufError)?;

        let delays = decode_u16s(&msg.Delays);
        let rotations = decode_u16s(&msg.Rotations);

        if delays.len() != rotations.len() {
            return Err(DigitalTouchError::ArraysDoNotMatch("delays".to_string(), delays.len(), "rotations".to_string(), rotations.len()));
        }

        Ok(DigitalTouchMessage::Kiss(DigitalTouchKiss{
            id: base_message.ID.clone(),
            kisses: merge_kiss_data(&msg.Points, &delays, &rotations)?,
        }))
    }
}

impl KissData {
    pub fn get_degs(&self) -> i16 {
        let rads = f64::from(self.rads) / 1000.0;
        let conv = 180.0 / PI;
        -((rads * conv) as i16)
    }
}

impl SVGTapRender<KissData> for DigitalTouchKiss {}

impl SVGTap<KissData> for DigitalTouchKiss {
    fn get_other_animations(&self, delay: usize) -> Vec<String> {
        vec![
            SVGCanvas::generate_elem("animate", HashMap::from([
                ("attributeName", "stroke-width".to_string()),
                ("values", "0.0; 25.0; 0.0".to_string()),
                ("keyTimes", "0; 0.25; 1".to_string()),
                ("dur", "1.5s".to_string()),
                ("begin", format!("{delay}ms")),
                ("repeatCount", "1".to_string()),
                ("restart", "whenNotActive".to_string()),
            ]), None),
        ]
    }

    fn get_point_elem(&self) -> &str {
        "path"
    }

    fn get_point_attrs(&self, point: &Point<KissData>, x: usize, y: usize) -> HashMap<&str, String> {
        let degs = point.extra.get_degs();
        HashMap::from([
            ("transform", format!("translate({x}, {y}) rotate({degs})")),
            ("d", r#"
M -50,0
L -14,-25
A 20,20 0 0,0 13,-25
L 50,0
L -50,0
A 40,20 0 0,0 50,0
"#.to_string()),
            ("fill", "none".to_string()),
            ("stroke", "red".to_string()),
            ("stroke-width", "0".to_string()),
            ("stroke-linecap", "round".to_string()),
            ("stroke-linejoin", "round".to_string()),
            ("opacity", "0".to_string())
        ])
    }

    fn get_points(&self) -> &Vec<Point<KissData>> {
        &self.kisses
    }

    fn get_point_delay(&self, p: &Point<KissData>) -> usize {
        p.extra.ms_delay as usize
    }
}

fn merge_kiss_data(points: &Vec<u8>, delays: &Vec<u16>, rotations: &Vec<u16>) -> Result<Vec<Point<KissData>>, DigitalTouchError> {
    decode_points(points, rotations.iter().zip(delays).map(|(&rads, &ms_delay)| {
        KissData{ rads, ms_delay }
    }).collect())
}