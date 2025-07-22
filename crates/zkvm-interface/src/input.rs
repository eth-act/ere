use std::{fmt::Debug, sync::Arc};

use bincode::Options;
use erased_serde::Serialize as ErasedSerialize;
use serde::Serialize;

#[derive(Clone)]
pub enum InputItem {
    /// A serializable object stored as a trait object
    Object(Arc<dyn ErasedSerialize + Send + Sync>),
    /// Pre-serialized bytes (e.g., from bincode)
    Bytes(Vec<u8>),
}

impl Debug for InputItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputItem::Object(_) => f.write_str("Object(<erased>)"),
            InputItem::Bytes(bytes) => f.debug_tuple("Bytes").field(bytes).finish(),
        }
    }
}

/// Represents a builder for input data to be passed to a ZKVM guest program.
#[derive(Debug, Clone)]
pub struct Input {
    items: Vec<InputItem>,
}
impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    /// Create an empty input buffer.
    pub fn new() -> Self {
        Self {
            items: Default::default(),
        }
    }

    /// Write a serializable value as a trait object
    pub fn write<T: Serialize + Send + Sync + 'static>(&mut self, value: T) {
        self.items.push(InputItem::Object(Arc::new(value)));
    }

    /// Write pre-serialized bytes directly
    pub fn write_bytes(&mut self, bytes: Vec<u8>) {
        self.items.push(InputItem::Bytes(bytes));
    }

    /// Get the number of items stored
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterate over the items
    pub fn iter(&self) -> std::slice::Iter<'_, InputItem> {
        self.items.iter()
    }
}

// Optional: Implement methods to work with the enum
impl InputItem {
    /// Serialize this item to bytes using the specified serializer
    pub fn serialize_with<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            InputItem::Object(obj) => erased_serde::serialize(obj.as_ref(), serializer),
            InputItem::Bytes(bytes) => {
                // Serialize the bytes as a byte array
                bytes.serialize(serializer)
            }
        }
    }

    /// Get the item as bytes (serialize objects, return bytes directly)
    pub fn as_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        match self {
            InputItem::Object(obj) => {
                let mut buf = Vec::new();
                let mut serializer = bincode::Serializer::new(
                    &mut buf,
                    bincode::DefaultOptions::new().with_fixint_encoding(),
                );
                erased_serde::serialize(obj.as_ref(), &mut serializer)?;
                Ok(buf)
            }
            InputItem::Bytes(bytes) => Ok(bytes.to_vec()),
        }
    }
}

#[cfg(test)]
mod input_erased_tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct Person {
        name: String,
        age: u32,
    }

    #[test]
    fn test_write_object() {
        let mut input = Input::new();

        let person = Person {
            name: "Alice".to_string(),
            age: 30,
        };

        input.write(person);
        assert_eq!(input.len(), 1);

        match &input.items[0] {
            InputItem::Object(_) => (), // Success
            InputItem::Bytes(_) => panic!("Expected Object, got Bytes"),
        }
    }

    #[test]
    fn test_write_bytes() {
        let mut input = Input::new();

        let bytes = vec![1, 2, 3, 4, 5];
        input.write_bytes(bytes.clone());

        assert_eq!(input.len(), 1);

        match &input.items[0] {
            InputItem::Bytes(stored_bytes) => assert_eq!(stored_bytes.to_vec(), bytes),
            InputItem::Object(_) => panic!("Expected Bytes, got Object"),
        }
    }

    #[test]
    fn test_write_serialized() {
        let mut input = Input::new();

        let person = Person {
            name: "Bob".to_string(),
            age: 25,
        };

        // User serializes themselves and writes bytes
        let serialized = bincode::serialize(&person).unwrap();
        input.write_bytes(serialized);

        assert_eq!(input.len(), 1);

        match &input.items[0] {
            InputItem::Bytes(_) => (), // Success
            InputItem::Object(_) => panic!("Expected Bytes, got Object"),
        }
    }

    #[test]
    fn test_mixed_usage() {
        let mut input = Input::new();

        let person = Person {
            name: "Charlie".to_string(),
            age: 35,
        };

        // Mix different write methods
        input.write(42i32); // Object
        let serialized = bincode::serialize(&person).unwrap();
        input.write_bytes(serialized); // Bytes (serialized)
        input.write_bytes(vec![10, 20, 30]); // Bytes (raw)
        input.write("hello".to_string()); // Object

        assert_eq!(input.len(), 4);

        // Verify types
        match &input.items[0] {
            InputItem::Object(_) => (),
            _ => panic!(),
        }
        match &input.items[1] {
            InputItem::Bytes(_) => (),
            _ => panic!(),
        }
        match &input.items[2] {
            InputItem::Bytes(_) => (),
            _ => panic!(),
        }
        match &input.items[3] {
            InputItem::Object(_) => (),
            _ => panic!(),
        }
    }

    #[test]
    fn test_as_bytes() {
        let mut input = Input::new();

        // Add an object
        input.write(42i32);

        // Add raw bytes
        input.write_bytes(vec![1, 2, 3]);

        // Convert both to bytes
        let obj_bytes = input.items[0].as_bytes().unwrap();
        let raw_bytes = input.items[1].as_bytes().unwrap();

        // The object should be serialized to some bytes
        assert!(!obj_bytes.is_empty());

        // The raw bytes should be returned as-is
        assert_eq!(raw_bytes, vec![1, 2, 3]);
    }

    #[test]
    fn test_iteration() {
        let mut input = Input::new();

        input.write(1);
        input.write(2);
        input.write_bytes(vec![3, 4, 5]);

        let count = input.iter().count();
        assert_eq!(count, 3);
    }
}
