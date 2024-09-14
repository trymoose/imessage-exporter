use protobuf::Message;
use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, TapMessage};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::message_types::digital_touch::models::Color;

const TAP_SVG_HEADER: &str = r#"<svg viewBox="0 0 250 250" width="100%" height="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalTouchTap {
    pub id: String,
    pub taps: Vec<TapPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapPoint {
    pub x: u16,
    pub y: u16,
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

    pub fn render_svg(&self) -> String {
        let mut svg = String::from(TAP_SVG_HEADER);
        svg.push_str(format!("<title>{}</title>\n", self.id).as_str());

        let mut delay = 1;
        for (index, tap) in self.taps.iter().enumerate() {
            delay += tap.ms_delay;
            svg.push_str(render_svg_tap(index, delay, tap).as_str())
        }

        svg.push_str("</svg>\n");
        svg
    }
}

fn render_svg_tap(index: usize, delay: u16, tap: &TapPoint) -> String {
    let (r, g, b, a) = tap.color.tuple();

    let x =  (f64::from(tap.x) / f64::from(u16::MAX)) * 100.0;
    let y =  (f64::from(tap.y) / f64::from(u16::MAX)) * 100.0;

    format!(r#"
<circle cx="{x:.2}%" cy="{y:.2}%" fill="none" stroke-width="30" stroke="rgba({r}, {g}, {b}, {a})" >
    <animate id="circle_{index}_size" attributeName="r" from="0%" to="50%" dur="1.5s" begin="{delay}ms" repeatCount="1" restart="whenNotActive" />
    <animate attributeName="opacity" values="0.0; 1.0; 0.0" keyTimes="0; 0.25; 1" dur="1.5s" begin="{delay}ms" repeatCount="1" restart="whenNotActive" />
</circle>
"#)
}

fn decode_color_buf(buf: &[u8]) -> Vec<Color> {
    let mut colors = vec![];
    let mut idx = 0;
    while idx < buf.len() {
        colors.push(Color{
            r: buf[idx],
            g: buf[idx+1],
            b: buf[idx+2],
            a: buf[idx+3],
        });
        idx += 4;
    }
    colors
}

fn decode_point_buf(buf: &[u8]) -> Vec<(u16, u16)> {
    let mut points = vec![];
    let mut idx = 0;
    while idx < buf.len() {
        points.push((
            u16::from_le_bytes([buf[idx], buf[idx+1]]),
            u16::from_le_bytes([buf[idx+2], buf[idx+3]]),
        ));
        idx += 4;
    }
    points
}

fn decode_delay_buf(buf: &[u8]) -> Vec<u16> {
    let mut delays = vec![];
    let mut idx = 0;
    while idx < buf.len() {
        delays.push(u16::from_le_bytes([buf[idx], buf[idx+1]]));
        idx += 2;
    }
    delays
}

fn merge_tap_data(points: Vec<(u16, u16)>, delays: Vec<u16>, colors: Vec<Color>) -> Vec<TapPoint> {
    let mut taps = vec![];
    for index in 0..colors.len() {
        let color = &colors[index];
        let (x, y) = points[index];
        taps.push(TapPoint{
            x, y,
            ms_delay: delays[index],
            color: color.clone(),
        });
    }
    taps
}