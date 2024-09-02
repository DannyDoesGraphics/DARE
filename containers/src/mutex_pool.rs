use std::sync::Mutex;

pub struct MutexPool<T> {
    pool: Vec<Mutex<T>>,
}
