use std::any::TypeId;
use std::boxed::Box;
use std::collections::HashMap;

/// A HashMap which has type erasure
pub type ErasedHashMap<T> = HashMap<TypeId, Box<T>>;
