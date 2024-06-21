/*!
 Contains logic to parse detailed data from `typedstream` data, focussing specifically on [NSAttributedString](https://developer.apple.com/documentation/foundation/nsattributedstring) data.

 Logic referenced from `typedstream` source located at:
   - [`typedstream.h`](https://opensource.apple.com/source/gcc/gcc-1493/libobjc/objc/typedstream.h.auto.html)
   - [`archive.c`](https://opensource.apple.com/source/gcc/gcc-5484/libobjc/archive.c.auto.html)
   - [`objc/typedstream.m`](https://archive.org/details/darwin_0.1)
*/

use crate::error::typedstream::TypedStreamError;

/// Indicates an [i16] in the byte stream
const I_16: u8 = 0x81;
/// Indicates an [i32] in the byte stream
const I_32: u8 = 0x82;
/// Indicates a float in the byte stream, the encoding determines the size
const DECIMAL: u8 = 0x83;
/// Indicates the start of a new object
const START: u8 = 0x84;
/// No data to parse, possibly end of an inheritance chain
const EMPTY: u8 = 0x85;
/// Indicates the last byte of an object
const END: u8 = 0x86;
/// When scanning for objects, bytes larger than the reference tag indicate an index in the table of already-seen types
const REFERENCE_TAG: i64 = 0x92;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    name: String,
    version: i64,
}

impl Class {
    fn new(name: String, version: i64) -> Self {
        Self { name, version }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputData {
    String(String),
    Integer(i64),
    Float(f32),
    Double(f64),
    Byte(u8),
    Class(Class),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Archivable {
    /// An instance of a class that may contain some data
    Object(Class, Vec<OutputData>),
    /// Some data that is likely a field on the object described by the `typedstream` but not part of a class
    Data(Vec<OutputData>),
    /// A class referenced in the `typedstream`
    Class(Class),
    /// A placeholder, only used when reserving a spot in the objects table for a reference to be filled with read class information.
    /// In a `typedstream`, the classes are stored in order of inheritance, so the top-level class described by the `typedstream`
    /// comes before the ones it inherits from. To preserve the order, we reserve the first slot to store the actual object's data
    /// and then later add it back to the right place.
    Placeholder,
}

// TODO: Remove clone
#[derive(Debug, Clone)]
enum Type {
    Utf8String,
    EmbeddedData,
    Object,
    SignedInt,
    UnsignedInt,
    Float,
    Double,
    String(String),
    Unknown(u8),
}

#[derive(Debug)]
enum ClassResult {
    Index(usize),
    ClassHierarchy(Vec<Archivable>),
}

impl Type {
    fn from_byte(byte: &u8) -> Self {
        match byte {
            0x40 => Self::Object,
            0x2B => Self::Utf8String,
            0x2A => Self::EmbeddedData,
            0x66 => Self::Float,
            0x64 => Self::Double,
            0x69 | 0x6c | 0x71 | 0x73 => Self::UnsignedInt,
            0x49 | 0x4c | 0x51 | 0x53 => Self::SignedInt,
            other => Self::Unknown(*other),
        }
    }

    fn new_string(string: String) -> Self {
        Self::String(string)
    }
}

#[derive(Debug)]
pub struct TypedStreamReader<'a> {
    /// The typedstream we want to parse
    stream: &'a [u8],
    /// The current index we are at in the stream
    idx: usize,
    /// As we parse the `typedstream`, build a table of seen types to reference in the future
    types_table: Vec<Vec<Type>>,
    /// As we parse the `typedstream`, build a table of seen archivable data to reference in the future
    object_table: Vec<Archivable>,
    /// Used to store the position of the current [Archivable::Placeholder]
    placeholder: Option<usize>,
}

impl<'a> TypedStreamReader<'a> {
    pub fn new(stream: &'a [u8]) -> Self {
        Self {
            stream,
            idx: 0,
            types_table: vec![],
            object_table: vec![],
            placeholder: None,
        }
    }

    // TODO: Remove
    fn emit_objects_table(&self) {
        println!("Start types table");
        self.types_table
            .iter()
            .enumerate()
            .for_each(|(idx, types)| println!("\t{idx}: {types:?}"));
        println!("End types table");
        println!("Start objects table");
        self.object_table
            .iter()
            .enumerate()
            .for_each(|(idx, obj)| println!("\t{idx}: {obj:?}"));
        println!("End objects table");
    }

    // TODO: Remove
    fn print_loc(&self, name: &str) {
        println!(
            "{name}: {:x}: {:x}",
            self.idx,
            self.get_current_byte().unwrap()
        );
    }

    /// Read an unsigned integer from the stream. Because we don't know the size of the integer ahead of time,
    /// we store it in the largest possible value.
    fn read_int(&mut self) -> Result<i64, TypedStreamError> {
        match self.get_current_byte()? {
            I_16 => {
                let size = 2;
                self.idx += 1;
                let value = u16::from_le_bytes(
                    self.stream
                        .get(self.idx..self.idx + size)
                        .ok_or(TypedStreamError::OutOfBounds(
                            self.idx + size,
                            self.stream.len(),
                        ))?
                        .try_into()
                        .map_err(TypedStreamError::SliceError)?,
                );
                self.idx += size;
                Ok(value as i64)
            }
            I_32 => {
                let size = 4;
                self.idx += 1;
                let value = u32::from_le_bytes(
                    self.stream
                        .get(self.idx..self.idx + size)
                        .ok_or(TypedStreamError::OutOfBounds(
                            self.idx + size,
                            self.stream.len(),
                        ))?
                        .try_into()
                        .map_err(TypedStreamError::SliceError)?,
                );
                self.idx += size;
                Ok(value as i64)
            }
            _ => {
                let value = u8::from_le_bytes([self.get_current_byte()?]);
                self.idx += 1;
                Ok(value as i64)
            }
        }
    }

    /// Read a single-precision float from the byte stream
    fn read_float(&mut self) -> Result<f32, TypedStreamError> {
        let size = 4;
        self.idx += 1;
        let value = f32::from_le_bytes(
            self.stream
                .get(self.idx..self.idx + size)
                .ok_or(TypedStreamError::OutOfBounds(
                    self.idx + size,
                    self.stream.len(),
                ))?
                .try_into()
                .map_err(TypedStreamError::SliceError)?,
        );
        self.idx += size;
        Ok(value)
    }

    /// Read a double-precision float from the byte stream
    fn read_double(&mut self) -> Result<f64, TypedStreamError> {
        let size = 8;
        self.idx += 1;
        let value = f64::from_le_bytes(
            self.stream
                .get(self.idx..self.idx + size)
                .ok_or(TypedStreamError::OutOfBounds(
                    self.idx + size,
                    self.stream.len(),
                ))?
                .try_into()
                .map_err(TypedStreamError::SliceError)?,
        );
        self.idx += size;
        Ok(value)
    }

    /// Read exactly `n` bytes from the stream
    fn read_exact_bytes(&mut self, n: usize) -> Result<&[u8], TypedStreamError> {
        let range = &self.stream[self.idx..self.idx + n];
        self.idx += n;
        Ok(range)
    }

    /// Read `n` bytes as a String
    fn read_exact_as_string(
        &mut self,
        n: usize,
        string: &mut String,
    ) -> Result<(), TypedStreamError> {
        let str = std::str::from_utf8(self.read_exact_bytes(n)?)
            .map_err(TypedStreamError::StringParseError)?;
        string.push_str(str);
        Ok(())
    }

    fn get_byte(&self, byte_idx: usize) -> Result<u8, TypedStreamError> {
        if byte_idx < self.stream.len() {
            return Ok(self.stream[byte_idx]);
        }
        Err(TypedStreamError::OutOfBounds(byte_idx, self.stream.len()))
    }

    /// Read the current byte
    fn get_current_byte(&self) -> Result<u8, TypedStreamError> {
        self.get_byte(self.idx)
    }

    /// Read the next byte
    fn get_next_byte(&self) -> Result<u8, TypedStreamError> {
        self.get_byte(self.idx + 1)
    }

    /// Determine the current types
    fn read_type(&mut self) -> Result<Vec<Type>, TypedStreamError> {
        let length = self.read_int();
        println!("type length: {length:?}");
        Ok(self
            .read_exact_bytes(length? as usize)?
            .iter()
            .map(Type::from_byte)
            .collect())
    }

    /// Read a reference pointer for a Type
    fn read_pointer(&mut self) -> Result<u32, TypedStreamError> {
        self.print_loc("pointer");
        let result = self.get_current_byte()? as u32 - REFERENCE_TAG as u32;
        self.idx += 1;
        Ok(result)
    }

    /// Read a class
    fn read_class(&mut self) -> Result<ClassResult, TypedStreamError> {
        let mut out_v: Vec<Archivable> = vec![];
        match self.get_current_byte()? {
            START => {
                // Skip some header bytes
                self.print_loc("class 1");
                while self.get_current_byte()? == START {
                    self.idx += 1;
                }
                self.print_loc("class 2");
                let length = self.read_int()?;

                if length >= REFERENCE_TAG {
                    let index = length - REFERENCE_TAG;
                    println!("Getting referenced class at {index}");
                    return Ok(ClassResult::Index(index as usize));
                }

                let mut class_name = String::with_capacity(length as usize);
                println!("Class name created with capacity {}", class_name.capacity());
                self.read_exact_as_string(length as usize, &mut class_name)?;

                let version = self.read_int()?;
                println!("{class_name} v{version}");
                println!("{}: {:?}", self.idx, self.get_current_byte());

                self.types_table
                    .push(vec![Type::new_string(class_name.clone())]);

                out_v.push(Archivable::Class(Class::new(class_name, version)));

                if let ClassResult::ClassHierarchy(parent) = self.read_class()? {
                    out_v.extend(parent);
                }
            }
            EMPTY => {
                self.idx += 1;
                println!("End of class chain!");
            }
            _ => {
                let index = self.read_pointer()?;
                println!("Getting referenced object at {index}");
                return Ok(ClassResult::Index(index as usize));
            }
        }
        Ok(ClassResult::ClassHierarchy(out_v))
    }

    /// read an object
    fn read_object(&mut self) -> Result<Option<&Archivable>, TypedStreamError> {
        match self.get_current_byte()? {
            START => {
                match self.read_class()? {
                    ClassResult::Index(idx) => {
                        return Ok(self.object_table.get(idx));
                    }
                    ClassResult::ClassHierarchy(classes) => {
                        for class in classes.iter() {
                            self.object_table.push(class.clone())
                        }
                    }
                }
                Ok(None)
            }
            EMPTY => {
                self.idx += 1;
                println!("Got empty object!");
                Ok(None)
            }
            _ => {
                let index = self.read_pointer()?;
                Ok(self.object_table.get(index as usize))
            }
        }
    }

    /// Read String data
    fn read_string(&mut self) -> Result<String, TypedStreamError> {
        let length = self.read_int()?;
        let mut string = String::with_capacity(length as usize);
        println!("String created with capacity {}", string.capacity());
        self.read_exact_as_string(length as usize, &mut string)?;

        Ok(string)
    }

    /// [Archivable] data can be embedded on a class or in a C String
    fn read_embedded_data(&mut self) -> Result<Option<Archivable>, TypedStreamError> {
        // Skip the 0x84
        self.idx += 1;
        match self.get_type()? {
            Some(types) => self.read_types(types),
            None => Ok(None),
        }
    }

    /// Gets the current type from the stream, either by reading it from the stream or reading it from
    /// the specified index of the types table. Because methods that use this type can also mutate self,
    /// returning a reference here means other methods could make that reference to the table invalid,
    /// which is disallowed in Rust. Thus, we return a clone of the cached data.
    fn get_type(&mut self) -> Result<Option<Vec<Type>>, TypedStreamError> {
        match self.get_current_byte()? {
            START => {
                // Ignore repeated types, for example in a dict
                self.idx += 1;

                let object_types = self.read_type()?;
                self.types_table.push(object_types);
                Ok(self.types_table.last().cloned())
            }
            END => {
                // This indicates the end of the current object
                Ok(None)
            }
            _ => {
                // Ignore repeated types, for example in a dict
                while self.get_current_byte()? == self.get_next_byte()? {
                    self.idx += 1;
                }

                let ref_tag = self.read_pointer()?;
                Ok(self.types_table.get(ref_tag as usize).cloned())
            }
        }
    }

    /// Given some types, look at the stream and parse the data according to the specified type
    fn read_types(
        &mut self,
        found_types: Vec<Type>,
    ) -> Result<Option<Archivable>, TypedStreamError> {
        let mut out_v = vec![];
        let mut is_obj: bool = false;

        for found_type in found_types {
            match found_type {
                Type::Utf8String => out_v.push(OutputData::String(self.read_string()?)),
                Type::EmbeddedData => {
                    return self.read_embedded_data();
                }
                Type::Object => {
                    is_obj = true;
                    let length = self.object_table.len();
                    self.placeholder = Some(length);
                    println!("Adding placeholder at {:?}", self.placeholder);
                    self.object_table.push(Archivable::Placeholder);
                    println!("Reading object...");
                    self.print_loc("reading object at");
                    if let Some(object) = self.read_object()? {
                        match object.clone() {
                            Archivable::Object(cls, data) => {
                                // If this is a new class, i.e. one without any data, we handle it later
                                // If the class already has data in it, we just want to use that class
                                // And put the data we found inside of it
                                if !data.is_empty() {
                                    self.object_table[length] =
                                        Archivable::Object(cls.clone(), vec![]);
                                }
                                out_v.extend(data)
                            }
                            Archivable::Class(cls) => out_v.push(OutputData::Class(cls)),
                            Archivable::Data(data) => out_v.extend(data),
                            Archivable::Placeholder => {
                                unreachable!()
                            } // This case should not hit
                        }
                    } else {
                        println!("NO OBJECT?");
                    }
                }
                Type::SignedInt => out_v.push(OutputData::Integer(self.read_int()?)),
                Type::UnsignedInt => out_v.push(OutputData::Integer(self.read_int()?)),
                Type::Float => out_v.push(OutputData::Float(self.read_float()?)),
                Type::Double => out_v.push(OutputData::Double(self.read_double()?)),
                Type::Unknown(byte) => out_v.push(OutputData::Byte(byte)),
                Type::String(s) => out_v.push(OutputData::String(s)),
            };
        }

        // If we had reserved a place for an object, fill that spot
        if let Some(spot) = self.placeholder {
            if !out_v.is_empty() {
                println!("Inserting {out_v:?} to object table at {spot}");
                // We got a class, but do not have its respective data yet
                if let Some(OutputData::Class(class)) = out_v.last() {
                    println!("Got output class {class:?}");
                    self.object_table[spot] = Archivable::Object(class.clone(), vec![]);
                // The spot after the current placeholder contains the class at the top of the class heirarchy, i.e.
                // if we get a placeholder and then find a new class heirarchy, the object table holds the class chain
                // in descending order of inheritance
                } else if let Some(Archivable::Class(class)) = self.object_table.get(spot + 1) {
                    println!("Got archived class {class:?}");
                    self.object_table[spot] = Archivable::Object(class.clone(), out_v.clone());
                    self.placeholder = None;
                    return Ok(self.object_table.get(spot).cloned());
                // We got some data for a class that was already seen
                } else if let Some(Archivable::Object(_, data)) = self.object_table.last_mut() {
                    println!("Got archived object");
                    data.extend(out_v.clone());
                    self.placeholder = None;
                    return Ok(self.object_table.last().cloned());
                // We got some data that is not part of a class, i.e. a field in the parent object for which we don't know the name
                } else {
                    println!("Got archived data");
                    self.object_table[spot] = Archivable::Data(out_v.clone());
                    self.placeholder = None;
                    return Ok(self.object_table.get(spot).cloned());
                }
            }
        }

        // TODO: This, but only for non-objects? Clean this logic up
        if !out_v.is_empty() && !is_obj {
            return Ok(Some(Archivable::Data(out_v.clone())));
        }
        Ok(None)
    }

    fn validate_header(&mut self) -> Result<(), TypedStreamError> {
        // Encoding type
        let typedstream_version = self.read_int()?;
        // Encoding signature
        let signature = self.read_string()?;
        self.idx += 1;
        // System version
        let system_version = self.read_int()?;

        if typedstream_version != 4 || signature != "streamtyped" || system_version != 232 {
            return Err(TypedStreamError::InvalidHeader);
        }

        self.idx += 1;

        Ok(())
    }

    /// Attempt to get the data from the typed stream
    pub fn parse(&mut self) -> Result<Vec<Archivable>, TypedStreamError> {
        let mut out_v = vec![];

        self.validate_header()?;

        while self.idx < self.stream.len() {
            if self.get_current_byte()? == END {
                println!("End of object!");
                self.idx += 1;
                continue;
            }

            println!("Parsed data: {:?}\n", out_v);

            // First, get the current type
            if let Some(found_types) = self.get_type()? {
                println!("Received types: {:?}", found_types);

                let result = self.read_types(found_types);
                println!("Resultant type: {result:?}");
                self.emit_objects_table();
                println!("Types table: {:?}", self.types_table);
                if let Ok(Some(res)) = result {
                    out_v.push(res);
                }
            }
        }

        self.emit_objects_table();
        println!("Types table: {:?}", self.types_table);
        println!("Parsed data: {:?}\n", out_v);
        Ok(out_v)
    }
}

#[cfg(test)]
mod tests {
    use std::env::current_dir;
    use std::fs::File;
    use std::io::Read;
    use std::vec;

    use crate::util::typedstream::{Archivable, Class, OutputData, TypedStreamReader};

    #[test]
    fn test_parse_header() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/AttributedBodyTextOnly");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.validate_header();

        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_text_mention() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/mentions/Mention");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse().unwrap();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("{item:?}"));

        let expected = vec![
            Archivable::Object(
                Class {
                    name: "NSMutableString".to_string(),
                    version: 1,
                },
                vec![OutputData::String("Test Dad ".to_string())],
            ),
            Archivable::Data(vec![OutputData::Integer(1), OutputData::Integer(5)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(0)],
            ),
            Archivable::Data(vec![OutputData::Integer(2), OutputData::Integer(3)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMentionConfirmedMention".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String("+15558675309".to_string())],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(0)],
            ),
            Archivable::Data(vec![OutputData::Integer(1), OutputData::Integer(1)]),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_text_basic() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/AttributedBodyTextOnly");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse().unwrap();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("{item:?}"));

        let expected = vec![
            Archivable::Object(
                Class {
                    name: "NSMutableString".to_string(),
                    version: 1,
                },
                vec![OutputData::String("Noter test".to_string())],
            ),
            Archivable::Data(vec![OutputData::Integer(1), OutputData::Integer(10)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(0)],
            ),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_text_basic_2() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/AttributedBodyTextOnly2");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse().unwrap();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("\t{item:?}"));

        let expected = vec![
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String("Test 3".to_string())],
            ),
            Archivable::Data(vec![OutputData::Integer(1), OutputData::Integer(6)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMBaseWritingDirectionAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(157)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(0)],
            ),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_text_long() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/LongMessage");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse().unwrap();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("{item:?}"));

        let expected = vec![
            Archivable::Data(vec![OutputData::Integer(1), OutputData::Integer(2359)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(0)],
            ),
        ];

        assert_eq!(result[1..], expected);
    }

    #[test]
    fn test_parse_text_multi_part() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/Multipart");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse().unwrap();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("\t{item:?}"));
        println!("\n\n");

        let expected = vec![
            Archivable::Object(
                Class {
                    name: "NSMutableString".to_string(),
                    version: 1,
                },
                vec![OutputData::String("￼test 1￼test 2 ￼test 3".to_string())],
            ),
            Archivable::Data(vec![OutputData::Integer(1), OutputData::Integer(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMFileTransferGUIDAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "at_0_F0668F79-20C2-49C9-A87F-1B007ABB0CED".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(0)],
            ),
            Archivable::Data(vec![OutputData::Integer(2), OutputData::Integer(6)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Data(vec![OutputData::Integer(3), OutputData::Integer(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMFileTransferGUIDAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "at_2_F0668F79-20C2-49C9-A87F-1B007ABB0CED".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
            Archivable::Data(vec![OutputData::Integer(4), OutputData::Integer(7)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(3)],
            ),
            Archivable::Data(vec![OutputData::Integer(5), OutputData::Integer(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMFileTransferGUIDAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "at_4_F0668F79-20C2-49C9-A87F-1B007ABB0CED".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(4)],
            ),
            Archivable::Data(vec![OutputData::Integer(6), OutputData::Integer(6)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(5)],
            ),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_text_multi_part_deleted() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/typedstream/MultiPartWithDeleted");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse().unwrap();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("\t{item:?}"));

        let expected = vec![
            Archivable::Object(
                Class {
                    name: "NSMutableString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "From arbitrary byte stream:\r￼To native Rust data structures:\r".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::Integer(1), OutputData::Integer(28)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(0)],
            ),
            Archivable::Data(vec![OutputData::Integer(2), OutputData::Integer(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMFileTransferGUIDAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "D0551D89-4E11-43D0-9A0E-06F19704E97B".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Data(vec![OutputData::Integer(3), OutputData::Integer(32)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSNumber".to_string(),
                    version: 0,
                },
                vec![OutputData::Integer(2)],
            ),
        ];

        println!("\n\nExpected data!");
        expected.iter().for_each(|item| println!("\t{item:?}"));
        println!("\n\n");

        assert_eq!(result, expected);
    }
}
