use genteel::memory::byte_utils::big_array;
use serde::ser::{SerializeTuple, Serializer, Error};
use std::fmt::Display;

struct MockSerializer;
struct MockError;

impl Error for MockError {
    fn custom<T: Display>(_msg: T) -> Self { MockError }
}
impl Display for MockError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }
}
impl std::fmt::Debug for MockError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }
}
impl std::error::Error for MockError {}

// Write tests here...
