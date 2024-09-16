/*!
 Errors that can happen when parsing `digital touch` data.
*/

use std::fmt::{Display, Formatter, Result};

/// Errors that can happen when parsing `digital touch` data
#[derive(Debug)]
pub enum DigitalTouchError {
    ProtobufError(protobuf::Error),
    UnknownDigitalTouchKind(i32),
    TapArraysDoNotMatch(usize, usize, usize),
    KissArraysDoNotMatch(usize, usize, usize),
}

impl Display for DigitalTouchError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        match self {
            DigitalTouchError::ProtobufError(why) => {
                write!(fmt, "failed to parse handwriting protobuf: {why}")
            }
            DigitalTouchError::UnknownDigitalTouchKind(kind) => {
                write!(fmt, "unknown digital touch kind: {kind}")
            }
            DigitalTouchError::TapArraysDoNotMatch(delays, point, color) => {
                write!(fmt, "length of arrays do not match: delays({delays}) != points({point}) != colors({color})")
            }
            DigitalTouchError::KissArraysDoNotMatch(delays, point, rotation) => {
                write!(fmt, "length of arrays do not match: delays({delays}) != points({point}) != rotations({rotation})")
            }
        }
    }
}
