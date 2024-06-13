/*!
 Contains logic to parse text from [NSAttributedString](https://developer.apple.com/documentation/foundation/nsattributedstring) data.
*/

use std::{char, usize, vec};

/// Indicates the start of a new object
const NEW_OBJECT_START: u8 = 0x0084;
/// Nil?
const NIL: u8 = 0x0085;
/// - Start of Selected Area> (SSA) <https://www.compart.com/en/unicode/U+0086>
const OBJECT_END: u8 = 0x0086;

/// Type encoding data
const ENCODING_DETECTED: u8 = 0x0095;
/// The "+" character
const NSSTRING_TYPE_ENCODING: u8 = 0x002b;
///  The "@" character
const OBJECT_TYPE_ENCODING: u8 = 0x040;

/// Tag data?
/// TODO: Unused
const FIRST_TAG: u8 = 0x0080;
const LAST_TAG: u8 = 0x0091;
const ZERO_TERMINATOR: u8 = 0x0000;
const ONE_TERMINATOR: u8 = 0x0001;
const SIGNED_OFFSET: u8 = 0x00ff;

// TODO: What are tags?
/// When scanning for objects, bytes >= reference tag indicate an index in the table of
/// already-seen types
const REFERENCE_TAG: u8 = 0x0092;

#[derive(Debug)]
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

// TODO: Remove clone
#[derive(Debug, Clone)]
enum ClassType {
    NSMutableAttributedString(u8),
    NSAttributedString(u8),
    NSObject(u8),
    NSMutableString(u8),
    NSString(u8),
    NSDictionary(u8),
    Unknown(String),
}

impl ClassType {
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
    Class(ClassType),
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
}

#[derive(Debug)]
struct StreamTypedReader<'a> {
    stream: &'a [u8],
    idx: usize,
}

impl<'a> StreamTypedReader<'a> {
    fn new(stream: &'a [u8]) -> Self {
        Self { stream, idx: 0 }
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

    /// Read a class object
    fn read_object(&mut self, types_table: &mut Vec<Vec<Type>>) -> Vec<Class> {
        let mut out_v = vec![];
        // Skip to the start of the object title
        println!("{} {:x}: {:?}", self.idx, self.idx, self.get_current_byte());
        while self.get_current_byte() != OBJECT_END {
            // TODO: The interior object gets added to the string table
            // TODO: before the parent, leading the string table to be misaligned
            while self.get_current_byte() == NEW_OBJECT_START {
                self.idx += 1
            }

            if self.get_current_byte() == NIL {
                println!("NIL found at {:x}", self.idx);
                self.idx += 1;
                continue;
            }

            if self.get_current_byte() == ENCODING_DETECTED {
                println!("Found some encoded data!");
                return out_v;
            }

            if self.get_current_byte() >= REFERENCE_TAG {
                println!("Object tag found: {:x}!", self.get_current_byte());
                let found_types = self.get_type(types_table);
                let read_types = self.read_types(found_types, types_table);
                out_v.push(Class::new(read_types, 100));
                continue;
            } else {
                let length = self.read_int();
                let mut class_name = String::with_capacity(length as usize);
                println!("Class created with capacity {}", class_name.capacity());
                self.read_exact_as_string(length as usize, &mut class_name);

                let version = self.read_int();
                println!("{class_name} v{version}");
                println!("{}: {:?}", self.idx, self.get_current_byte());
                let found_class = Class::new(class_name, version);
                let parsed_class = ClassType::from_class(&found_class);
                out_v.push(found_class);

                let parsed_type = Type::Class(parsed_class);
                println!("Got parsed type! {:?}", parsed_type);
                types_table.push(vec![parsed_type]);
            }
        }

        self.idx += 1;
        println!("out v -> {:?}", out_v);
        out_v
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
    fn handle_ns_string(&mut self, version: &u8, types_table: &mut Vec<Vec<Type>>) -> String {
        println!("Handling string data!");

        // TODO: Use real errors
        if version != &0x01 {
            print!("Parse error: unsupported version!");
            return String::new();
        }

        let mut out_s = String::new();
        if self.get_current_byte() == ENCODING_DETECTED {
            self.idx += 1;
            println!("Parsing encoded data!");
            // TODO: recurse here, or something to parse the encoded data
            let encodings = self.get_type(types_table);
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
        } else {
            print!("Parse error: no encoded data!");
            return String::new();
        }

        out_s
    }

    /// Parse custom NSDictionary data
    fn handle_ns_dict(&mut self, version: &u8, types_table: &mut Vec<Vec<Type>>) -> String {
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
        let length = self.get_type(types_table);
        println!("{:?}", length);
        for detected_type in length {
            if let Type::UnsignedInt = detected_type {
                dict_length = self.read_int();
            }
        }

        println!("NSDict with {:?} items", dict_length);
        self.idx += 1; // Skip 0x92
        self.idx += 1; // Skip 0x84
        println!("{:x}: {:x}", self.idx, self.get_current_byte());

        for _ in 0..dict_length {
            // Read the key and value types
            let key_types = self.get_type(types_table);
            // self.idx += 1; // TODO: Key types are repeated?
            let key_data = self.read_types(key_types, types_table);

            self.idx += 1; // TODO: Skip the 0x86 here?

            let value_types = self.get_type(types_table);
            let value_data = self.read_types(value_types, types_table);

            out_s.push_str(&format!("{key_data}: {value_data}"));
        }

        out_s
    }

    fn get_type(&mut self, types_table: &mut Vec<Vec<Type>>) -> Vec<Type> {
        match self.get_current_byte() {
            NEW_OBJECT_START => {
                println!("New type found!");

                self.idx += 1;
                let object_types = self.read_type();
                types_table.push(object_types);
                println!("Found types: {:?}", types_table);
                types_table.last().unwrap().to_owned()
            }
            OBJECT_END => {
                println!("End of current object!");
                return vec![];
            }
            _ => {
                // Ignore repeated types, for example in a dict
                while self.get_current_byte() == self.get_next_byte() {
                    self.idx += 1;
                }

                let ref_tag = self.read_pointer();
                let possible_types = types_table.get(ref_tag as usize).unwrap().clone();
                println!("Got referenced type {ref_tag}: {possible_types:?}");
                possible_types
            }
        }
    }

    fn read_types(&mut self, found_types: Vec<Type>, types_table: &mut Vec<Vec<Type>>) -> String {
        let mut out_s = String::new();

        for object_type in found_types {
            let result = match object_type {
                Type::Utf8String => self.read_string(),
                Type::NullTerminatedString => self.read_null_terminated_string(),
                Type::Object => {
                    let objects = self.read_object(types_table);
                    let mut out_s = format!("{:?}", objects);
                    for object in &objects {
                        let parsed_class = ClassType::from_class(object);
                        match &parsed_class {
                            ClassType::NSString(version) => {
                                out_s.push_str(&format!(
                                    ": {}",
                                    self.handle_ns_string(version, types_table)
                                ));
                            }
                            ClassType::NSDictionary(version) => {
                                out_s.push_str(&format!(
                                    ": {}",
                                    self.handle_ns_dict(version, types_table)
                                ));
                            }
                            other => {
                                println!("{other:?} does not have parsing rules!")
                            }
                        }
                    }
                    out_s
                }
                Type::SignedInt => format!("Signed: {} ", self.read_int()),
                Type::UnsignedInt => format!("Unsigned: {} ", self.read_int()),
                Type::Unknown(_) => todo!(),
                Type::Class(name) => match &name {
                    ClassType::NSString(version) => {
                        format!(": {}", self.handle_ns_string(version, types_table))
                    }
                    ClassType::NSDictionary(version) => {
                        format!(": {}", self.handle_ns_dict(version, types_table))
                    }
                    other => {
                        format!("{other:?} does not have parsing rules!")
                    }
                },
            };
            out_s.push_str(&result);
            continue;
        }
        out_s
    }

    /// Attempt to get the data from the typed stream
    fn parse(&mut self) -> Vec<String> {
        let mut out_v = vec![];
        let mut types_table: Vec<Vec<Type>> = vec![];

        // Skip header
        // TODO: Parse it
        self.idx += 16;

        while self.idx < self.stream.len() {
            println!("Parsed data: {:?}\n", out_v);

            // First, get the current type
            let found_types = self.get_type(&mut types_table);

            let result = self.read_types(found_types, &mut types_table);

            println!("Resultant type: {result}");
            out_v.push(result);

            if self.get_current_byte() == ENCODING_DETECTED {
                println!("\nFound new encoding!");
                self.idx += 1;
                continue;
            } else if self.get_current_byte() == OBJECT_END {
                println!("End of object!");
                self.idx += 1;
                continue;
            }
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

    use crate::util::attributed_string::StreamTypedReader;

    #[test]
    fn test_parse_text_mention() {
        let plist_path = current_dir()
            .unwrap()
            .as_path()
            .join("test_data/mentions/Mention");
        let mut file = File::open(plist_path).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();

        let mut parser = StreamTypedReader::new(&bytes);
        println!("{parser:?}");
        let result = parser.parse();

        println!("\n\nGot data!");
        result.iter().for_each(|item| println!("\n{item}"))

        // let expected = "Noter test".to_string();

        // assert_eq!(parsed, expected);
    }
}
