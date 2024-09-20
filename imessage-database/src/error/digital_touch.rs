/*!
 Errors that can happen when parsing `digital touch` data.
*/

use std::fmt::{Display, Formatter, Result};

/// Errors that can happen when parsing `digital touch` data
#[derive(Debug)]
pub enum DigitalTouchError {
    ProtobufError(protobuf::Error),
    UnknownDigitalTouchKind(i32),
    ArraysDoNotMatch(String, usize, String, usize),
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
            DigitalTouchError::ArraysDoNotMatch(n1, v1, n2, v2) => {
                write!(fmt, "length of arrays do not match: {n1}({v1}) != {n2}({v2})")
            }
        }
    }
}
