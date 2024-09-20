use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, SketchMessage};
use crate::message_types::digital_touch::models::{decode_points, Color, Point};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::util::svg_canvas::SVGCanvas;
use protobuf::Message;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub struct DigitalTouchSketch {
    pub id: String,
    pub strokes: Vec<Vec<Point<Color>>>,
}

impl DigitalTouchSketch {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = SketchMessage::parse_from_bytes(base_message.TouchPayload.as_slice()).map_err(DigitalTouchError::ProtobufError)?;

        let colors = Color::decode_all(&msg.Colors);

        let mut strokes = vec![vec![]; msg.StrokesCount as usize];
        let mut index = 0;
        for stroke in 0..(msg.StrokesCount as usize) {
            index += 2;
            let point_count = u16::from_le_bytes([msg.Strokes[index], msg.Strokes[index+1]]) as usize;
            index += 2;

            let num_bytes = point_count * 4;
            let stroke_color = vec![colors[stroke].clone(); point_count];
            strokes[stroke] = decode_points(&msg.Strokes[index..index+num_bytes], stroke_color)?;
            index += num_bytes;
        }

        Ok(DigitalTouchMessage::Sketch(DigitalTouchSketch {
            id: base_message.ID.clone(),
            strokes,
        }))
    }

    pub fn render_svg(&self, canvas: &mut SVGCanvas) {
        self.strokes.iter().for_each(|stroke| {
            let (r, g, b, a) = stroke[0].extra.tuple();

            canvas.write_elem("polyline", HashMap::from([
                ("points", stroke.iter().map(|point| {
                    let x = canvas.fit_x(point.x as usize, u16::MAX as usize);
                    let y = canvas.fit_y(point.y as usize, u16::MAX as usize);
                    format!("{x},{y}")
                }).collect::<Vec<String>>().join(" ")),
                ("stroke-width", "10px".to_string()),
                ("fill", "none".to_string()),
                ("stroke", format!("rgba({r}, {g}, {b}, {a})")),
            ]), None);
        });
    }
}