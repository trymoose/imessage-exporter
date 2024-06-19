/*!
 Errors that can happen when parsing `typedstream` data. This module is for the new `typedstream` parser.
*/

use std::fmt::{Display, Formatter, Result};

/// Errors that can happen when parsing `typedstream` data
#[derive(Debug)]
pub enum StreamTypedError {
    OutOfBounds(u8, u8),
    InvalidHeader,
}

impl Display for StreamTypedError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        match self {
            StreamTypedError::OutOfBounds(idx, len) => {
                write!(fmt, "Index {idx:x} is outside of range {len:x}!")
            }
            StreamTypedError::InvalidHeader => write!(fmt, "Invalid typedstream header!"),
        }
    }
}
