use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, DrawingMessage};
use crate::message_types::digital_touch::models::{Color, Point};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::util::ascii_canvas::AsciiCanvas;
use protobuf::Message;

#[derive(Debug, PartialEq, Eq)]
pub struct DigitalTouchDrawing {
    pub id: String,
    pub strokes: Vec<DrawingStroke>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DrawingStroke {
    pub color: Color,
    pub points: Vec<Point>,
}

impl DigitalTouchDrawing {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = DrawingMessage::parse_from_bytes(base_message.TouchPayload.as_slice()).map_err(DigitalTouchError::ProtobufError)?;

        let mut strokes = vec![];
        let mut index = 0;
        for stroke_num in 0..(msg.StrokesCount as usize) {
            let color = Color::from_bytes(&msg.Colors[(stroke_num*4)..(stroke_num*4)+4]);
            index += 2;
            let point_count = u16::from_le_bytes([msg.Strokes[index], msg.Strokes[index+1]]);
            index += 2;

            let mut points = vec![];
            for _ in 0..point_count {
                points.push(Point{
                    x: u16::from_le_bytes([msg.Strokes[index], msg.Strokes[index+1]]),
                    y: u16::from_le_bytes([msg.Strokes[index+2], msg.Strokes[index+3]]),
                });
                index += 4
            }
            strokes.push(DrawingStroke{ color, points });
        }

        Ok(DigitalTouchMessage::Drawing(DigitalTouchDrawing{
            id: base_message.ID.clone(),
            strokes,
        }))
    }

    pub fn render_svg(&self, size: usize) -> String {
        let mut svg = String::from(format!(r#"<svg viewBox="0 0 {size} {size}" width="100%" height="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
"#));
        svg.push_str(format!("<title>{}</title>\n", self.id).as_str());

        self.strokes.iter().for_each(|stroke| {
            let mut points = String::new();
            stroke.points.iter().for_each(|point| {
                points.push_str(format!("{},{} ", (point.x as u32 * size as u32) / u16::MAX as u32, (point.y as u32 * size as u32) / u16::MAX as u32).as_str());
            });
            let (r, g, b, a) = stroke.color.tuple();
            svg.push_str(format!(r#"<polyline points="{points}" stroke-width="10px" fill="none" stroke="rgba({r}, {g}, {b}, {a})" />
"#).as_str());
        });

        svg.push_str("</svg>\n");
        svg
    }

    pub fn render_ascii(&self, max_height: usize) -> String {
        let mut canvas = AsciiCanvas::new(max_height, max_height);
        self.strokes.iter().for_each(|stroke| {
            stroke.points.iter().map(|point| {
                Point{
                    x: (max_height as u16 * point.x) / u16::MAX,
                    y: (max_height as u16 * point.y) / u16::MAX,
                }
            }).collect::<Vec<Point>>().windows(2).for_each(|points| {
                canvas.draw_line(points[0].x, points[0].y, points[1].x, points[1].y);
            });
        });
        canvas.to_string()
    }
}