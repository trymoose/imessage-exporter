/*!
 Contains logic to parse detailed data from `typedstream` data, focussing specifically on [NSAttributedString](https://developer.apple.com/documentation/foundation/nsattributedstring) data.

 Derived from `typedstream` source located [here](https://opensource.apple.com/source/gcc/gcc-1493/libobjc/objc/typedstream.h.auto.html), [here](https://opensource.apple.com/source/gcc/gcc-5484/libobjc/archive.c.auto.html), and [here](https://sourceforge.net/projects/aapl-darwin/files/Darwin-0.1/objc-1.tar.gz/download)
*/

use crate::error::typedstream::TypedStreamError;

/// Indicates the start of a new object
const START: u8 = 0x0084;
/// No data to parse, possibly end of an inheritance chain
const EMPTY: u8 = 0x0085;
/// Indicates the last byte of an object
const END: u8 = 0x0086;
/// Type encoding data
const ENCODING_DETECTED: u8 = 0x0095;
/// When scanning for objects, bytes >= reference tag indicate an index in the table of already-seen types
const REFERENCE_TAG: u8 = 0x0092;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    name: String,
    version: u8,
    embedded_data: bool,
}

impl Class {
    fn new(name: String, version: u8, embedded_data: bool) -> Self {
        Self {
            name,
            version,
            embedded_data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputData {
    String(String),
    Number(i32),
    Byte(u8),
    Class(Class),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    String(String),
    Unknown(u8),
}

#[derive(Debug)]
enum ClassResult {
    Index(u8),
    ClassHierarchy(Vec<Archivable>),
}

impl Type {
    fn from_byte(byte: &u8) -> Self {
        match byte {
            0x0040 => Self::Object,
            0x002B => Self::Utf8String,
            0x002A => Self::EmbeddedData,
            0x0069 => Self::UnsignedInt,
            0x0049 => Self::SignedInt,
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

    /// Read the current byte as a signed integer
    fn read_int(&mut self) -> Result<u8, TypedStreamError> {
        let value = u8::from_le_bytes([self.get_current_byte()?]);
        self.idx += 1;
        Ok(value)
    }

    /// Read exactly `n` bytes from the stream
    fn read_exact_bytes(&mut self, n: usize) -> &[u8] {
        let range = &self.stream[self.idx..self.idx + n];
        self.idx += n;
        range
    }

    /// Read `n` bytes as a String
    /// TODO: Result<(), TypedStreamError>
    fn read_exact_as_string(&mut self, n: usize, string: &mut String) {
        let str = std::str::from_utf8(self.read_exact_bytes(n)).unwrap();
        string.push_str(str);
    }

    fn get_byte(&self, byte_idx: usize) -> Result<u8, TypedStreamError> {
        if byte_idx < self.stream.len() {
            return Ok(self.stream[byte_idx]);
        }
        return Err(TypedStreamError::OutOfBounds(byte_idx, self.stream.len()));
    }

    /// Read the current byte
    fn get_current_byte(&self) -> Result<u8, TypedStreamError> {
        self.get_byte(self.idx)
    }

    /// Read the next byte
    // TODO: Bounds check
    fn get_next_byte(&self) -> Result<u8, TypedStreamError> {
        self.get_byte(self.idx + 1)
    }

    /// Determine the current types
    fn read_type(&mut self) -> Result<Vec<Type>, TypedStreamError> {
        let length = self.read_int();
        println!("type length: {length:?}");
        Ok(self
            .read_exact_bytes(length? as usize)
            .iter()
            .map(Type::from_byte)
            .collect())
    }

    /// Read a reference pointer for a Type
    fn read_pointer(&mut self) -> Result<u8, TypedStreamError> {
        self.print_loc("pointer");
        let result = self.get_current_byte()? - REFERENCE_TAG;
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

                // TODO: this adds a new item to the object table, but it shouldn't
                if length >= REFERENCE_TAG {
                    let index = length - REFERENCE_TAG;
                    // TODO: this is a reference to a string, we should build a class with that name
                    // Or store the class as the Type
                    println!("Getting referenced class at {index}");
                    return Ok(ClassResult::Index(index));
                }

                let mut class_name = String::with_capacity(length as usize);
                println!("Class name created with capacity {}", class_name.capacity());
                self.read_exact_as_string(length as usize, &mut class_name);

                let version = self.read_int()?;
                println!("{class_name} v{version}");
                println!("{}: {:?}", self.idx, self.get_current_byte());

                self.types_table
                    .push(vec![Type::new_string(class_name.clone())]);

                out_v.push(Archivable::Class(Class::new(
                    class_name,
                    version,
                    self.get_current_byte()? == ENCODING_DETECTED,
                )));

                if let ClassResult::ClassHierarchy(parent) = self.read_class()? {
                    out_v.extend(parent);
                }
            }
            EMPTY => {
                self.idx += 1;
                println!("End of class chain!");
            }
            ENCODING_DETECTED => {
                self.idx += 1;
                println!("Encoded data up next!");
            }
            _ => {
                let index = self.read_pointer()?;
                println!("Getting referenced object at {index}");
                return Ok(ClassResult::Index(index));
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
                        return Ok(self.object_table.get(idx as usize));
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
        self.read_exact_as_string(length as usize, &mut string);

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

    fn get_type(&mut self) -> Result<Option<Vec<Type>>, TypedStreamError> {
        match self.get_current_byte()? {
            START => {
                println!("New type found!");
                // Ignore repeated types, for example in a dict
                self.idx += 1;
                let object_types = self.read_type()?;
                self.types_table.push(object_types);
                // self.object_table.push(Archivable::Object(vec![OutputData::NewObject]));
                println!("Found types: {:?}", self.types_table);
                Ok(Some(self.types_table.last().unwrap().to_owned()))
            }
            END => {
                println!("End of current object!");
                Ok(None)
            }
            _ => {
                // Ignore repeated types, for example in a dict
                while self.get_current_byte()? == self.get_next_byte()? {
                    self.idx += 1;
                }

                let ref_tag = self.read_pointer()?;
                let possible_types = self.types_table.get(ref_tag as usize).unwrap().clone();
                println!("Got referenced type {ref_tag}: {possible_types:?}");
                Ok(Some(possible_types))
            }
        }
    }

    fn read_types(
        &mut self,
        found_types: Vec<Type>,
    ) -> Result<Option<Archivable>, TypedStreamError> {
        let mut out_v = vec![];
        let mut is_obj: bool = false;

        for object_type in found_types {
            match object_type {
                Type::Utf8String => out_v.push(OutputData::String(self.read_string()?)),
                Type::EmbeddedData => {
                    return self.read_embedded_data();
                }
                Type::Object => {
                    is_obj = true;
                    self.placeholder = Some(self.object_table.len());
                    println!("Adding placeholder at {:?}", self.placeholder);
                    self.object_table.push(Archivable::Placeholder);
                    println!("Reading object...");
                    self.print_loc("reading object at");
                    let object = self.read_object()?;
                    println!("Got object {object:?}");
                    if let Some(object) = object {
                        match object.clone() {
                            Archivable::Object(_, data) => out_v.extend(data),
                            Archivable::Class(cls) => out_v.push(OutputData::Class(cls)),
                            Archivable::Placeholder => {}
                            Archivable::Data(data) => out_v.extend(data),
                        }
                    }
                }
                Type::SignedInt => out_v.push(OutputData::Number(self.read_int()? as i32)),
                Type::UnsignedInt => out_v.push(OutputData::Number(self.read_int()? as i32)),
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
                    println!("Got output class");
                    if class.embedded_data {
                        self.object_table[spot] = Archivable::Object(class.clone(), vec![])
                    } else {
                        self.object_table.remove(spot);
                        self.placeholder = None;
                    }
                // We got some data to fill a class that we have not seen yet
                } else if let Some(Archivable::Class(class)) = self.object_table.last() {
                    println!("Got archived class");
                    if class.embedded_data {
                        self.object_table[spot] = Archivable::Object(class.clone(), out_v.clone());
                    }
                    self.placeholder = None;
                    return Ok(self.object_table.get(spot).cloned());
                // We got some data for a class that was already seen
                } else if let Some(Archivable::Object(_, data)) = self.object_table.last_mut() {
                    println!("Got archived object");
                    data.extend(out_v.clone());
                    return Ok(self.object_table.last().cloned());
                // We got some data that is not part of a class, i.e. a field in the parent object for which we don't know the name
                } else {
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

    /// Attempt to get the data from the typed stream
    pub fn parse(&mut self) -> Result<Vec<Archivable>, TypedStreamError> {
        let mut out_v = vec![];

        // Skip header
        // TODO: Parse it
        self.idx += 16;

        while self.idx < self.stream.len() {
            if self.get_current_byte()? == END {
                println!("End of object!");
                self.idx += 1;
                continue;
            }

            println!("Parsed data: {:?}\n", out_v);

            // First, get the current type
            // TODO: remove unwrap
            let found_types = self.get_type()?;
            println!("Received types: {:?}", found_types);

            let result = self.read_types(found_types.unwrap());
            println!("Resultant type: {result:?}");
            if let Ok(res) = result {
                match res {
                    Some(output) => {
                        out_v.push(output);
                    }
                    None => {}
                }
            }
            self.emit_objects_table();
            println!("Types table: {:?}", self.types_table);
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
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String("Test Dad ".to_string())],
            ),
            Archivable::Data(vec![OutputData::Number(1), OutputData::Number(5)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSValue".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(0)],
            ),
            Archivable::Data(vec![OutputData::Number(2), OutputData::Number(3)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMentionConfirmedMention".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String("+15558675309".to_string())],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::Number(0)]),
            Archivable::Data(vec![OutputData::Number(1), OutputData::Number(1)]),
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
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String("Noter test".to_string())],
            ),
            Archivable::Data(vec![OutputData::Number(1), OutputData::Number(10)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSValue".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(0)],
            ),
        ];

        assert_eq!(result, expected);
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
        result.iter().for_each(|item| println!("{item:?}"));

        let expected = vec![
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String("￼test 1￼test 2 ￼test 3".to_string())],
            ),
            Archivable::Data(vec![OutputData::Number(1), OutputData::Number(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMFileTransferGUIDAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "at_0_F0668F79-20C2-49C9-A87F-1B007ABB0CED".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSValue".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(0)],
            ),
            Archivable::Data(vec![OutputData::Number(2), OutputData::Number(6)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::Number(1)]),
            Archivable::Data(vec![OutputData::Number(3), OutputData::Number(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(2)],
            ),
            Archivable::Data(vec![OutputData::String(
                "__kIMFileTransferGUIDAttributeName".to_string(),
            )]),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "at_2_F0668F79-20C2-49C9-A87F-1B007ABB0CED".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::String(
                "__kIMMessagePartAttributeName".to_string(),
            )]),
            Archivable::Data(vec![OutputData::Number(2)]),
            Archivable::Data(vec![OutputData::Number(4), OutputData::Number(7)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(1)],
            ),
            Archivable::Data(vec![OutputData::String(
                "__kIMMessagePartAttributeName".to_string(),
            )]),
            Archivable::Data(vec![OutputData::Number(3)]),
            Archivable::Data(vec![OutputData::Number(5), OutputData::Number(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(2)],
            ),
            Archivable::Data(vec![OutputData::String(
                "__kIMFileTransferGUIDAttributeName".to_string(),
            )]),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "at_4_F0668F79-20C2-49C9-A87F-1B007ABB0CED".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::String(
                "__kIMMessagePartAttributeName".to_string(),
            )]),
            Archivable::Data(vec![OutputData::Number(4)]),
            Archivable::Data(vec![OutputData::Number(6), OutputData::Number(6)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(1)],
            ),
            Archivable::Data(vec![OutputData::String(
                "__kIMMessagePartAttributeName".to_string(),
            )]),
            Archivable::Data(vec![OutputData::Number(5)]),
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
        result.iter().for_each(|item| println!("{item:?}"));

        let expected = vec![
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "From arbitrary byte stream:\r￼To native Rust data structures:\r".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::Number(1), OutputData::Number(28)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSValue".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(0)],
            ),
            Archivable::Data(vec![OutputData::Number(2), OutputData::Number(1)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(2)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMFileTransferGUIDAttributeName".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "D0551D89-4E11-43D0-9A0E-06F19704E97B".to_string(),
                )],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::Number(1)]),
            Archivable::Data(vec![OutputData::Number(3), OutputData::Number(32)]),
            Archivable::Object(
                Class {
                    name: "NSDictionary".to_string(),
                    version: 0,
                    embedded_data: true,
                },
                vec![OutputData::Number(1)],
            ),
            Archivable::Object(
                Class {
                    name: "NSString".to_string(),
                    version: 1,
                    embedded_data: true,
                },
                vec![OutputData::String(
                    "__kIMMessagePartAttributeName".to_string(),
                )],
            ),
            Archivable::Data(vec![OutputData::Number(2)]),
        ];

        assert_eq!(result, expected);
    }
}
