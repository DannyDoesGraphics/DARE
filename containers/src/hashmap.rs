use std::any::TypeId;
use std::collections::HashMap;
use std::boxed::Box;

/// A HashMap which has type erasure
pub type ErasedHashMap<T> = HashMap<TypeId, Box<T>>;