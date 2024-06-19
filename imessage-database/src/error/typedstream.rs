/*!
 Errors that can happen when parsing `typedstream` data. This module is for the new `typedstream` parser.
*/

use std::{
    array::TryFromSliceError,
    fmt::{Display, Formatter, Result},
};

/// Errors that can happen when parsing `typedstream` data
#[derive(Debug)]
pub enum TypedStreamError {
    OutOfBounds(usize, usize),
    InvalidHeader,
    SliceError(TryFromSliceError),
}

impl Display for TypedStreamError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        match self {
            TypedStreamError::OutOfBounds(idx, len) => {
                write!(fmt, "Index {idx:x} is outside of range {len:x}!")
            }
            TypedStreamError::InvalidHeader => write!(fmt, "Invalid typedstream header!"),
            TypedStreamError::SliceError(why) => {
                write!(fmt, "Unable to slice source stream: {why}")
            }
        }
    }
}
