use serde::{Serializer, Serialize};
use genteel::memory::byte_utils::big_array;
use serde::ser::Error;

struct BadSerializer;

impl Serializer for BadSerializer {
    type Ok = ();
    type Error = serde::de::value::Error;
    type SerializeSeq = serde::ser::Impossible<(), Self::Error>;
    type SerializeTuple = serde::ser::Impossible<(), Self::Error>;
    // ...
}
