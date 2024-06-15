/*!
 Contains logic to parse detailed data from `typedstream` data, focussing specifically on [NSAttributedString](https://developer.apple.com/documentation/foundation/nsattributedstring) data.

 Derived from `typedstream` source located [here](https://opensource.apple.com/source/gcc/gcc-1493/libobjc/objc/typedstream.h.auto.html) and [here](https://sourceforge.net/projects/aapl-darwin/files/Darwin-0.1/objc-1.tar.gz/download)
*/

use std::{char, collections::HashMap, usize, vec};

/// Indicates the start of a new object
const START: u8 = 0x0084;
/// No data to parse, possibly end of an inheritance chain
const EMPTY: u8 = 0x0085;
/// Indicates the last byte of an object
const END: u8 = 0x0086;

/// Type encoding data
const ENCODING_DETECTED: u8 = 0x0095;

// TODO: What are tags?
/// When scanning for objects, bytes >= reference tag indicate an index in the table of
/// already-seen types
const REFERENCE_TAG: u8 = 0x0092;

#[derive(Debug, Clone)]
struct Class {
    name: String,
    version: u8,
}

impl Class {
    fn new(name: String, version: u8) -> Self {
        Self { name, version }
    }

    fn as_string(&self) -> String {
        return format!("{} v{}", self.name, self.version);
    }
}

#[derive(Debug)]
enum OutputData {
    String(String),
    Number(i32),
    Dict(HashMap<String, String>),
    None,
}

#[derive(Debug, Clone)]
enum Archivable {
    Object(Object),
    Class(Class),
}

// TODO: Remove clone
#[derive(Debug, Clone)]
enum Object {
    NSMutableAttributedString(u8),
    NSAttributedString(u8),
    NSObject(u8),
    NSMutableString(u8),
    NSString(u8),
    NSDictionary(u8),
    Unknown(String),
}

impl Object {
    fn from_class(class: &Class) -> Self {
        match class.name.as_str() {
            "NSMutableAttributedString" => Self::NSMutableAttributedString(class.version),
            "NSAttributedString" => Self::NSAttributedString(class.version),
            "NSObject" => Self::NSObject(class.version),
            "NSMutableString" => Self::NSMutableString(class.version),
            "NSString" => Self::NSString(class.version),
            "NSDictionary" => Self::NSDictionary(class.version),
            // TODO: Remove copy
            _ => Self::Unknown(class.name.to_owned()),
        }
    }
}

// TODO: Remove clone
#[derive(Debug, Clone)]
enum Type {
    Utf8String,
    NullTerminatedString,
    Object,
    SignedInt,
    UnsignedInt,
    ObjectName(Object),
    String(String),
    Unknown(u8),
}

impl Type {
    fn from_byte(byte: &u8) -> Self {
        match byte {
            0x0040 => Self::Object,
            0x002B => Self::Utf8String,
            0x002A => Self::NullTerminatedString,
            0x0069 => Self::UnsignedInt,
            0x0049 => Self::SignedInt,
            other => Self::Unknown(*other),
        }
    }

    fn new_class(class: Object) -> Self {
        Self::ObjectName(class)
    }

    fn new_string(string: String) -> Self {
        Self::String(string)
    }
}

#[derive(Debug)]
struct TypedStreamReader<'a> {
    stream: &'a [u8],
    idx: usize,
    string_table: Vec<Vec<Type>>,
    object_table: Vec<Archivable>,
}

impl<'a> TypedStreamReader<'a> {
    fn new(stream: &'a [u8]) -> Self {
        Self {
            stream,
            idx: 0,
            string_table: vec![],
            object_table: vec![],
        }
    }

    /// Read the current byte as a signed integer
    fn read_int(&mut self) -> u8 {
        let value = u8::from_le_bytes([self.get_current_byte()]);
        self.idx += 1;
        value
    }

    /// Read exactly `n` bytes from the stream
    fn read_exact_bytes(&mut self, n: usize) -> &[u8] {
        let range = &self.stream[self.idx..self.idx + n];
        self.idx += n;
        range
    }

    /// Read `n` bytes as a String
    fn read_exact_as_string(&mut self, n: usize, string: &mut String) {
        let str = std::str::from_utf8(self.read_exact_bytes(n)).unwrap();
        string.push_str(str);
    }

    /// Read the current byte
    fn get_current_byte(&self) -> u8 {
        self.stream[self.idx]
    }

    /// Read the next byte
    // TODO: Bounds check
    fn get_next_byte(&self) -> u8 {
        self.stream[self.idx + 1]
    }

    /// Determine the current types
    fn read_type(&mut self) -> Vec<Type> {
        let length = self.read_int();
        println!("type length: {length}");
        self.read_exact_bytes(length as usize)
            .iter()
            .map(Type::from_byte)
            .collect()
    }

    /// Read a reference pointer for a Type
    fn read_pointer(&mut self) -> u8 {
        let result = self.get_current_byte() - REFERENCE_TAG;
        self.idx += 1;
        result
    }

    /// Read a class
    fn read_class(&mut self) -> Option<&Archivable> {
        match self.get_current_byte() {
            START => {
                // Skip some header bytes
                while self.get_current_byte() == START {
                    self.idx += 1;
                }
                let length = self.read_int();
                let mut class_name = String::with_capacity(length as usize);
                println!("Class name created with capacity {}", class_name.capacity());
                self.read_exact_as_string(length as usize, &mut class_name);

                let version = self.read_int();
                println!("{class_name} v{version}");
                println!("{}: {:?}", self.idx, self.get_current_byte());

                let found_class = Class::new(class_name, version);
                let parsed_class = Object::from_class(&found_class);
                self.string_table
                    .push(vec![Type::new_class(parsed_class.clone())]);
                self.object_table.push(Archivable::Object(parsed_class));

                self.read_class()?;
                self.object_table.last()
            }
            EMPTY => {
                self.idx += 1;
                None
            }
            _ => {
                let index = self.read_pointer();
                self.object_table.get(index as usize)
            }
        }
    }

    /// read an object
    fn read_object(&mut self) -> Option<&Archivable> {
        match self.get_current_byte() {
            START => {
                if let Some(obj_class) = self.read_class() {
                    return Some(obj_class);
                }
                None
            }
            EMPTY => {
                self.idx += 1;
                None
            }
            _ => {
                let index = self.read_pointer();
                self.object_table.get(index as usize)
            }
        }
    }

    /// Read String data
    fn read_string(&mut self) -> String {
        let length = self.read_int();
        let mut string = String::with_capacity(length as usize);
        println!("String created with capacity {}", string.capacity());
        self.read_exact_as_string(length as usize, &mut string);

        string
    }

    fn read_null_terminated_string(&mut self) -> String {
        let mut out_s = String::new();
        println!("{:x}", self.get_current_byte());
        while self.get_current_byte() != 0x0000 {
            out_s.push(char::from_u32(self.get_current_byte() as u32).unwrap());
            self.idx += 1;
        }
        // Skip the null byte at the end
        self.idx += 1;

        out_s
    }

    /// Parse custom NSString data
    fn handle_ns_string(&mut self, version: &u8) -> String {
        println!("Handling string data!");

        // TODO: Use real errors
        if version != &0x01 {
            print!("Parse error: unsupported version!");
            return String::new();
        }

        let mut out_s = String::new();
        println!("Parsing encoded data!");
        // TODO: recurse here, or something to parse the encoded data
        let encodings = self.get_type();
        for encoding in encodings {
            if let Type::Utf8String = encoding {
                let result = self.read_string();
                println!("NSString Parsed {result} for {encoding:?}");
                out_s.push_str(&result);
            } else {
                print!("Parse error: malformed encoded data type!");
                return String::new();
            }
        }

        out_s
    }

    /// Parse custom NSDictionary data
    fn handle_ns_dict(&mut self, version: &u8) -> String {
        println!("Handling dict data!");
        let mut out_s = String::new();

        // TODO: Use real errors
        if version != &0x00 {
            print!("Parse error: unsupported version!");
            return String::new();
        }

        // Read the size of the dict (the number of {key: val} pairs)
        let mut dict_length: u8 = 0;
        if self.get_current_byte() == ENCODING_DETECTED {
            self.idx += 1;
        }
        let length = self.get_type();
        for detected_type in length {
            if let Type::UnsignedInt = detected_type {
                dict_length = self.read_int();
            }
        }

        println!("NSDict with {:?} items", dict_length);
        // self.idx += 1; // Skip 0x92
        // self.idx += 1; // Skip 0x84
        println!("{:x}: {:x}", self.idx, self.get_current_byte());

        for id in 0..dict_length {
            // Read the key and value types
            println!(
                "Dict item {id} - {:x}: {:x}",
                self.idx,
                self.get_current_byte()
            );

            // Read the key and value types
            self.idx += 2; // TODO: Key types are repeated?
            let key_types = self.get_type();
            let key_data = self.read_types(key_types);
            println!("Got key: {key_data:?}");

            let value_types = self.get_type();
            let value_data = self.read_types(value_types);
            println!("Got val: {value_data:?}");

            println!("{id}, {}", &format!("{key_data:?}: {value_data:?}"));

            out_s.push_str(&format!("{key_data:?}: {value_data:?}"));
        }

        out_s
    }

    fn get_type(&mut self) -> Vec<Type> {
        match self.get_current_byte() {
            START => {
                println!("New type found!");
                // Ignore repeated types, for example in a dict
                while self.get_next_byte() == START {
                    self.idx += 1;
                }

                self.idx += 1;
                let object_types = self.read_type();
                // types_table.push(object_types);
                self.string_table.push(object_types);
                println!("Found objects: {:?}", self.object_table);
                println!("Found types: {:?}", self.string_table);
                self.string_table.last().unwrap().to_owned()
            }
            END => {
                println!("End of current object!");
                vec![]
            }
            _ => {
                // Ignore repeated types, for example in a dict
                while self.get_current_byte() == self.get_next_byte() {
                    self.idx += 1;
                }

                let ref_tag = self.read_pointer();
                let possible_types = self.string_table.get(ref_tag as usize).unwrap().clone();
                println!("Got referenced type {ref_tag}: {possible_types:?}");
                possible_types
            }
        }
    }

    fn read_types(&mut self, found_types: Vec<Type>) -> Vec<OutputData> {
        let mut out_v = vec![];
        for object_type in found_types {
            let res = match object_type {
                Type::Utf8String => OutputData::String(self.read_string()),
                Type::NullTerminatedString => {
                    OutputData::String(self.read_null_terminated_string())
                }
                Type::Object => {
                    println!("Reading object...");
                    let object = self.read_object();
                    println!("Got object {object:?}");
                    if let Some(object) = object {
                        match object.clone() {
                            Archivable::Object(obj) => match obj {
                                Object::NSString(version) => {
                                    OutputData::String(self.handle_ns_string(&version))
                                }
                                Object::NSDictionary(version) => {
                                    OutputData::String(self.handle_ns_dict(&version))
                                }
                                other => OutputData::String(format!(
                                    "{other:?} does not have parsing rules!"
                                )),
                            },
                            Archivable::Class(cls) => OutputData::String(cls.as_string()),
                        }
                    } else {
                        OutputData::None
                    }
                }
                Type::SignedInt => OutputData::Number(self.read_int() as i32),
                Type::UnsignedInt => OutputData::Number(self.read_int() as i32),
                Type::Unknown(_) => todo!(),
                Type::ObjectName(name) => match &name {
                    Object::NSString(version) => {
                        OutputData::String(self.handle_ns_string(&version))
                    }
                    Object::NSDictionary(version) => {
                        OutputData::String(self.handle_ns_dict(&version))
                    }
                    other => OutputData::String(format!("{other:?} does not have parsing rules!")),
                },
                Type::String(s) => OutputData::String(s),
            };
            out_v.push(res);
            continue;
        }
        out_v
    }

    /// Attempt to get the data from the typed stream
    fn parse(&mut self) -> Vec<Vec<OutputData>> {
        let mut out_v = vec![];

        // Skip header
        // TODO: Parse it
        self.idx += 16;

        while self.idx < self.stream.len() {
            if self.get_current_byte() == ENCODING_DETECTED {
                println!("\nFound new encoding!");
                self.idx += 1;
                continue;
            } else if self.get_current_byte() == END {
                println!("End of object!");
                self.idx += 1;
                continue;
            }

            println!("Parsed data: {:?}\n", out_v);

            // First, get the current type
            let found_types = self.get_type();
            println!("Received types: {:?}", found_types);

            let result = self.read_types(found_types);
            println!("Resultant type: {result:?}");

            out_v.push(result);
        }

        println!("Parsed data: {:?}\n", out_v);
        out_v
    }
}

#[cfg(test)]
mod tests {
    use std::env::current_dir;
    use std::fs::File;
    use std::io::Read;
    use std::vec;

    use crate::util::attributed_string::TypedStreamReader;

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
        let result = parser.parse();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("\n{item:?}"))

        // let expected = "Noter test".to_string();

        // assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_text_basic() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/streamtyped/AttributedBodyTextOnly");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = TypedStreamReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("\n{item:?}"))

        // let expected = "Noter test".to_string();

        // assert_eq!(parsed, expected);
    }
}
