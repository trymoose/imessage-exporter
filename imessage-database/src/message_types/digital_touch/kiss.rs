use std::fmt::Write;
use std::f64::consts::PI;
use protobuf::Message;
use crate::error::digital_touch::DigitalTouchError;
use crate::message_types::digital_touch::digital_touch_proto::{BaseMessage, KissMessage};
use crate::message_types::digital_touch::DigitalTouchMessage;
use crate::message_types::digital_touch::models::{decode_bytes, Point};
use crate::util::svg_canvas::SVGCanvas;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalTouchKiss {
    pub id: String,
    pub kisses: Vec<KissPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KissPoint {
    pub point: Point,
    pub ms_delay: u16,
    pub rads: u16,
}

impl DigitalTouchKiss {
    pub(crate) fn from_payload(base_message: &BaseMessage) -> Result<DigitalTouchMessage, DigitalTouchError> {
        let msg = KissMessage::parse_from_bytes(base_message.TouchPayload.as_slice()).map_err(DigitalTouchError::ProtobufError)?;

        let delays = decode_bytes(&msg.Delays, 2, |buf| u16::from_le_bytes([buf[0], buf[1]]));
        let rotations = decode_bytes(&msg.Rotations, 2, |buf| u16::from_le_bytes([buf[0], buf[1]]));
        let points = decode_bytes(&msg.Points, 4, |buf| (
            u16::from_le_bytes([buf[0], buf[1]]),
            u16::from_le_bytes([buf[2], buf[3]]),
        ));

        if delays.len() != rotations.len() || rotations.len() != points.len() {
            return Err(DigitalTouchError::KissArraysDoNotMatch(delays.len(), points.len(), rotations.len()));
        }

        Ok(DigitalTouchMessage::Kiss(DigitalTouchKiss{
            id: base_message.ID.clone(),
            kisses: delays.iter().zip(rotations).zip(points).map(|((&delay, rads), (x, y))| KissPoint{
                point: Point{ x, y: u16::MAX - y },
                ms_delay: delay,
                rads,
            }).collect(),
        }))
    }

    pub fn render_svg(&self, canvas: &mut SVGCanvas) {
        let mut delay = 1;
        self.kisses.iter().for_each(|kiss| {
            delay += kiss.ms_delay;
            render_svg_kiss(canvas, delay, kiss);
        });
    }
}

impl KissPoint {
    pub fn get_degs(&self) -> i16 {
        let rads = f64::from(self.rads) / 1000.0;
        let conv = 180.0 / PI;
        -((rads * conv) as i16)
    }
}

fn render_svg_kiss(canvas: &mut SVGCanvas, delay: u16, kiss: &KissPoint) {
    let x =  canvas.fit_x(kiss.point.x as usize, u16::MAX as usize);
    let y =  canvas.fit_y(kiss.point.y as usize, u16::MAX as usize);
    let degs = kiss.get_degs();

    let _ = writeln!(canvas, r#"<path transform="translate({x},{y}) rotate({degs})" d="
    M -50,0
    L -14,-25
    A 20,20 0 0,0 13,-25
    L 50,0
    L -50,0
    A 40,20 0 0,0 50,0
    " fill="none" stroke="red" stroke-width="0" stroke-linecap="round" stroke-linejoin="round" opacity="0" >
        <animate attributeName="opacity" values="0.0; 1.0; 0.0" keyTimes="0; 0.25; 1" dur="1.5s" begin="{delay}ms" repeatCount="1" restart="whenNotActive" />
        <animate attributeName="stroke-width" values="0.0; 25.0; 0.0" keyTimes="0; 0.25; 1" dur="1.5s" begin="{delay}ms" repeatCount="1" restart="whenNotActive" />
</path>"#);
}