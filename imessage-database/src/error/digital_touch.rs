/*!
 Errors that can happen when parsing `digital touch` data.
*/

use std::fmt::{Display, Formatter, Result};

/// Errors that can happen when parsing `digital touch` data
#[derive(Debug)]
pub enum DigitalTouchError {
    ProtobufError(protobuf::Error),
}

impl Display for DigitalTouchError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        match self {
            DigitalTouchError::ProtobufError(why) => {
                write!(fmt, "failed to parse handwriting protobuf: {why}")
            }
        }
    }
}
