use std::collections::HashMap;
use protobuf::Message;
use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, HeartbeatMessage};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::util::svg_canvas::SVGCanvas;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalTouchHeartbeat {
    pub id: String,
    pub bpm: usize,
    pub duration: usize,
    pub heart_broken_at_ms: usize,
}

impl DigitalTouchHeartbeat {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = HeartbeatMessage::parse_from_bytes(&base_message.TouchPayload).map_err(DigitalTouchError::ProtobufError)?;

        Ok(DigitalTouchMessage::Heartbeat(DigitalTouchHeartbeat{
            id: base_message.ID.clone(),
            bpm: msg.BPM as usize,
            duration: msg.Duration as usize,
            heart_broken_at_ms: (msg.HeartBrokenAt * 1000.0) as usize,
        }))
    }

    pub fn render_svg(&self, canvas: &mut SVGCanvas) {
        let mut beats_in_interval = ((self.bpm * 1000) * (self.duration * 1000)) / 60000;
        if beats_in_interval % 1000 != 0 {
            beats_in_interval = ((beats_in_interval / 1000) + 1) * 1000;
        }
        beats_in_interval /= 1000;
        let time_per_beat = (self.duration * 1000) / beats_in_interval;

        let x = canvas.fit_x(1, 2) - 50;
        let y = canvas.fit_y(1, 2) - 45;

        canvas.write_elem("g", HashMap::from([
            ("transform", format!("translate({x},{y})")),
        ]), Some(vec![
            SVGCanvas::generate_elem("metadata", HashMap::new(), Some(vec![
                SVGCanvas::generate_elem("bpm", HashMap::new(), Some(format!("{}", self.bpm))),
                SVGCanvas::generate_elem("duration", HashMap::from([("unit", "second".to_string())]), Some(format!("{}", self.duration))),
            ].join("\n"))),
            SVGCanvas::generate_elem("path", HashMap::from([
                ("transform-origin", "50 45".to_string()),
                ("fill", "red".to_string()),
                ("fill-opacity", "40%".to_string()),
                ("stroke", "red".to_string()),
                ("stroke-width", format!("{}", 15)),
                ("stroke-linecap", "round".to_string()),
                ("stroke-linejoin", "round".to_string()),
                ("d", r#"
                    M 0,30
                    L 50,90
                    L 100,30
                    A 25,30 0 0,0 50,30
                    A 25,30 0 0,0 0,30
                "#.to_string()),
            ]), Some(SVGCanvas::generate_elem("animateTransform", HashMap::from([
                ("attributeName", "transform".to_string()),
                ("type", "scale".to_string()),
                ("begin", "0s".to_string()),
                ("values", "1.0; 2.0; 1.0".to_string()),
                ("keyTimes", "0; 0.70; 1".to_string()),
                ("dur", format!("{}ms", time_per_beat)),
                ("repeatCount", format!("{beats_in_interval}")),
                ("restart", "whenNotActive".to_string()),
            ]), None))),
        ].join("\n")));
    }
}