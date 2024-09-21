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

    pub fn beats_in_interval(&self) -> usize {
        let mut beats_in_interval = ((self.bpm * 1000) * (self.duration * 1000)) / 60000;
        if beats_in_interval % 1000 != 0 {
            beats_in_interval = ((beats_in_interval / 1000) + 1) * 1000;
        }
        beats_in_interval / 1000
    }

    pub fn time_per_beat(&self) -> usize{
        (self.duration * 1000) / self.beats_in_interval()
    }

    pub fn render_svg(&self, canvas: &mut SVGCanvas) {
        let x = canvas.fit_x(1, 2) - 50;
        let y = canvas.fit_y(1, 2) - 45;

        let mut heart_sides = vec![];
        vec![HeartSide::Left(), HeartSide::Right()].iter().for_each(|side| {
            heart_sides.push(SVGCanvas::generate_elem("path", HashMap::from([
                ("d", side.d()),
                ("transform-origin", "50 45".to_string()),
                ("fill", "red".to_string()),
                ("stroke", "red".to_string()),
                ("stroke-width", "5".to_string()),
                ("stroke-linecap", "round".to_string()),
                ("stroke-dasharray", "264.304".to_string()),
                ("stroke-dashoffset", "99.617".to_string()),
            ]), Some(vec![
                self.get_animations(side, x, y),
            ].join("\n"))));
        });

        canvas.write_elem("g", HashMap::from([
            ("transform", format!("translate({x},{y})")),
        ]), Some(vec![
            self.get_metadata(),
            heart_sides.join("\n"),
        ].join("\n")));
    }

    fn get_metadata(&self) -> String {
        let mut metadata = vec![
            SVGCanvas::generate_elem("id", HashMap::new(), Some(self.id.clone())),
            SVGCanvas::generate_elem("bpm", HashMap::new(), Some(format!("{}", self.bpm))),
            SVGCanvas::generate_elem("duration", HashMap::from([("unit", "second".to_string())]), Some(format!("{}", self.duration))),
        ];

        if self.heart_broken_at_ms > 0 {
            metadata.push(SVGCanvas::generate_elem(
                "heartbreak",
                HashMap::from([("unit", "millisecond".to_string())]),
                Some(format!("{}", self.heart_broken_at_ms))));
        }
        SVGCanvas::generate_elem("metadata", HashMap::new(), Some(metadata.join("\n")))
    }

    fn get_animations(&self, side: &HeartSide, x: usize, y: usize) -> String {
        let beats_in_interval = self.beats_in_interval();
        let time_per_beat = self.time_per_beat();

        vec![
            vec![
                SVGCanvas::generate_elem("animateTransform", HashMap::from([
                    ("attributeName", "transform".to_string()),
                    ("type", "scale".to_string()),
                    ("begin", "0s".to_string()),
                    ("values", "1.0; 2.0; 1.0".to_string()),
                    ("keyTimes", "0; 0.70; 1".to_string()),
                    ("dur", format!("{}ms", time_per_beat)),
                    ("repeatCount", format!("{beats_in_interval}")),
                    ("restart", "whenNotActive".to_string()),
                ]), None),
            ],
            self.get_heartbreak_animation(side, x, y),
        ].concat().join("\n")
    }

    fn get_heartbreak_animation(&self, side: &HeartSide, x: usize, y: usize) -> Vec<String> {
        let mut heartbreak = vec![];
        if self.heart_broken_at_ms > 0 {
            heartbreak.push(
                SVGCanvas::generate_elem("animateTransform", HashMap::from([
                    ("attributeName", "transform".to_string()),
                    ("type", "scale".to_string()),
                    ("begin", format!("{}ms", self.heart_broken_at_ms+1)),
                    ("from", "1.0".to_string()),
                    ("to", "0.75".to_string()),
                    ("fill", "freeze".to_string()),
                    ("dur", "1.5s".to_string()),
                    ("repeatCount", 1.to_string()),
                    ("restart", "whenNotActive".to_string()),
                ]), None),
            );

            heartbreak.push(
                SVGCanvas::generate_elem("animateTransform", HashMap::from([
                    ("attributeName", "transform".to_string()),
                    ("type", "rotate".to_string()),
                    ("begin", format!("{}ms", self.heart_broken_at_ms+1)),
                    ("from", "0 50 60".to_string()),
                    ("to", format!("{} 50 60", side.rotate_deg())),
                    ("fill", "freeze".to_string()),
                    ("dur", "1.5s".to_string()),
                    ("repeatCount", 1.to_string()),
                    ("restart", "whenNotActive".to_string()),
                ]), None),
            );
        }
        heartbreak
    }
}

enum HeartSide {
    Left(),
    Right(),
}

impl HeartSide {
    fn d(&self) -> String {
        match self {
            HeartSide::Right() => "
    M 50,30
    A 25,30 0 0,1 100,30
    L 50,90
    L 39,60
    L 64,50
    L 39,40
    L 50,30
  ".to_string(),
            HeartSide::Left() => "
    M 50,30
    A 25,30 0 0,0 0,30
    L 50,90
    L 41,60
    L 66,50
    L 41,40
    L 50,30
    ".to_string(),
        }
    }
    fn rotate_deg(&self) -> isize {
        match self {
            HeartSide::Left() => -15,
            HeartSide::Right() => 15,
        }
    }

    fn drop_amount(&self) -> usize {
        match self {
            HeartSide::Left() => 5,
            HeartSide::Right() => 10,
        }
    }
}