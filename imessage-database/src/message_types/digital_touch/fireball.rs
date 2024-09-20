use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, FireballMessage};
use crate::message_types::digital_touch::models::{decode_points, decode_u16s, Point};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::message_types::digital_touch::DigitalTouchMessage::Fireball;
use crate::util::svg_canvas::SVGCanvas;
use protobuf::Message;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalTouchFireball {
    pub id: String,
    pub x: u16,
    pub y: u16,
    pub points: Vec<Point<u16>>,
    pub duration: usize,
}

impl DigitalTouchFireball {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = FireballMessage::parse_from_bytes(&base_message.TouchPayload).map_err(DigitalTouchError::ProtobufError)?;

        let delays = decode_u16s(&msg.Delays);

        Ok(Fireball(DigitalTouchFireball{
            id: base_message.ID.clone(),
            x: convert_offset(msg.StartX),
            y: convert_offset(msg.StartY),
            points: decode_points(&msg.Points, delays)?,
            duration: (msg.Duration * 1000.0) as usize,
        }))
    }

    pub fn render_svg(&self, canvas: &mut SVGCanvas) {
        add_radial_gradient_def(canvas);

        canvas.write_elem("circle", HashMap::from([
            ("cx", format!("{}", 0)),
            ("cy", format!("{}", 0)),
            ("r", format!("{}", 50)),
            ("fill", "url('#fireball-gradient')".to_string()),
        ]), Some(self.render_svg_animations(canvas)));
    }

    fn render_svg_animations(&self, canvas: &SVGCanvas) -> String {
        let mut delay = self.get_initial_delay();


        let min_y = self.points.iter().map(|p| p.y).fold(self.y, |acc, y| acc.min(y));
        let max_y = self.points.iter().map(|point| point.y - min_y).fold(self.y - min_y, |acc, y| acc.max(y));
        let mut prev_x = canvas.fit_x(self.x as usize, u16::MAX as usize);
        let tmp_y = canvas.fit_y(self.y as usize - min_y as usize, max_y as usize);
        let mut prev_y = canvas.height() - tmp_y;


        vec![
            SVGCanvas::generate_elem("animateMotion", HashMap::from([
                ("dur", format!("{}ms", 1)),
                ("begin", format!("{}ms", 1)),
                ("repeatCount", 1.to_string()),
                ("path", format!("M {prev_x},{prev_y} L {prev_x},{prev_y}")),
            ]), None),
            self.points.iter().map(|point| {
                let x = canvas.fit_x(point.x as usize, u16::MAX as usize);
                let tmp_y = canvas.fit_y((point.y - min_y) as usize, max_y as usize);
                let y = canvas.height() - tmp_y;
                let elem = SVGCanvas::generate_elem("animateMotion", HashMap::from([
                    ("dur", format!("{}ms", point.extra)),
                    ("begin", format!("{delay}ms")),
                    ("repeatCount", 1.to_string()),
                    ("path", format!("M {prev_x},{prev_y} L {x},{y}")),
                ]), None);
                delay += point.extra as usize;
                prev_x = x;
                prev_y = y;
                elem
            }).collect::<Vec<String>>().join("\n"),
        ].join("\n")
    }

    fn get_initial_delay(&self) -> usize {
        self.points.iter().map(|p| p.extra as i64).fold(self.duration as i64, |acc, e| acc - e).max(0) as usize
    }
}

fn add_radial_gradient_def(canvas: &mut SVGCanvas) {
    canvas.add_def("fireball-gradient", "radialGradient", HashMap::new(), Some(vec![
        SVGCanvas::generate_elem("stop", HashMap::from([
            ("offset", format!("{}%", 0)),
            ("stop-color", "white".to_string()),
        ]), None),
        SVGCanvas::generate_elem("stop", HashMap::from([
            ("offset", format!("{}%", 30)),
            ("stop-color", "gold".to_string()),
        ]), None),
        SVGCanvas::generate_elem("stop", HashMap::from([
            ("offset", format!("{}%", 60)),
            ("stop-color", "orange".to_string()),
        ]), None),
        SVGCanvas::generate_elem("stop", HashMap::from([
            ("offset", format!("{}%", 100)),
            ("stop-color", "red".to_string()),
        ]), None),
    ].join("\n")));
}

fn convert_offset(offset: f32) -> u16 {
    (offset * (u16::MAX as f32)).floor() as u16
}