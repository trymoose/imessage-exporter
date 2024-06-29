/*!
 Data structures used to parse `typedstream` data, focussing specifically on [NSAttributedString](https://developer.apple.com/documentation/foundation/nsattributedstring) data.
*/

/// Represents a class stored in the `typedstream`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// The name of the class
    pub name: String,
    /// The encoded version of the class
    pub version: u64,
}

impl Class {
    pub(crate) fn new(name: String, version: u64) -> Self {
        Self { name, version }
    }
}

/// Rust structures containing data stored in the `typedstream`
#[derive(Debug, Clone, PartialEq)]
pub enum OutputData {
    /// Text data
    String(String),
    /// Signed integer types are coerced into this container
    SignedInteger(i64),
    /// Unsigned integer types are coerced into this container
    UnsignedInteger(u64),
    /// Floating point numbers
    Float(f32),
    /// Double precision floats
    Double(f64),
    /// Bytes whose type is not known
    Byte(u8),
    /// Arbitrary collection of bytes in an array
    Array(Vec<u8>),
    /// A found class, in order of inheritance
    Class(Class),
}

/// Types of data that can be archived into the `typedstream`
#[derive(Debug, Clone, PartialEq)]
pub enum Archivable {
    /// An instance of a class that may contain some embedded data
    Object(Class, Vec<OutputData>),
    /// Some data that is likely a field on the object described by the `typedstream` but not part of a class
    Data(Vec<OutputData>),
    /// A class referenced in the `typedstream`, usually part of an inheritance heirarchy that does not contain any data itself
    Class(Class),
    /// A placeholder, only used when reserving a spot in the objects table for a reference to be filled with read class information.
    /// In a `typedstream`, the classes are stored in order of inheritance, so the top-level class described by the `typedstream`
    /// comes before the ones it inherits from. To preserve the order, we reserve the first slot to store the actual object's data
    /// and then later add it back to the right place.
    Placeholder,
    Type(Vec<Type>),
}

/// Represents types of data that can be stored in a `typedstream`
///
/// Types are cached in [`TypedStreamReader::types_table`]; the first time one is seen they are present
/// in the stream literally, but afterwards are only referenced by index in order of appearance.
// TODO: Remove clone
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Encoded string data, usually embedded in an object
    Utf8String,
    /// Encoded bytes that can be parsed again as data
    EmbeddedData,
    /// An instance of a class, usually with data
    Object,
    /// An [`i8`], [`i16`], or [`i32`]
    SignedInt,
    /// A [`u8`], [`u16`], or [`u32`]
    UnsignedInt,
    /// An [`f32`]
    Float,
    /// An [`f64`]
    Double,
    /// Some text we can reuse later, i.e. a class name
    String(String),
    /// An array containing some data of a given length
    Array(usize),
    /// Data for which we do not know the type, likely for something this parser does not implement
    Unknown(u8),
}

/// Represents data that results from attempting to parse a class from the `typedstream`
#[derive(Debug)]
pub(crate) enum ClassResult {
    /// A reference to an already-seen class in the [`TypedStreamReader::object_table`]
    Index(usize),
    /// A new class heirarchy to be inserted into the [`TypedStreamReader::object_table`]
    ClassHierarchy(Vec<Archivable>),
}

impl Type {
    pub(crate) fn from_byte(byte: &u8) -> Self {
        match byte {
            0x40 => Self::Object,
            0x2B => Self::Utf8String,
            0x2A => Self::EmbeddedData,
            0x66 => Self::Float,
            0x64 => Self::Double,
            0x69 | 0x6c | 0x71 | 0x73 => Self::SignedInt,
            0x49 | 0x4c | 0x51 | 0x53 => Self::UnsignedInt,
            other => Self::Unknown(*other),
        }
    }

    pub(crate) fn new_string(string: String) -> Self {
        Self::String(string)
    }

    pub(crate) fn get_array_length(types: &[u8]) -> Option<Vec<Type>> {
        if types.first() == Some(&0x5b) {
            let len =
                types[1..]
                    .iter()
                    .take_while(|a| a.is_ascii_digit())
                    .fold(None, |acc, ch| {
                        char::from_u32(*ch as u32)?
                            .to_digit(10)
                            .map(|b| acc.unwrap_or(0) * 10 + b)
                    })?;
            return Some(vec![Type::Array(len as usize)]);
        }
        None
    }
}
