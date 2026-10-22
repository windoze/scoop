//! Borrowed views serialize explicitly without copying the semantic graph.

use serde::{Serialize, Serializer, ser::SerializeSeq};

/// Protocol projection of an already established semantic value.
pub struct QueryJson<'a, T: ?Sized>(pub &'a T);

macro_rules! object {
    ($serializer:expr, {$($name:literal: $value:expr),* $(,)?}) => {{
        use serde::ser::SerializeMap;
        let mut map = $serializer.serialize_map(Some([$($name),*].len()))?;
        $(map.serialize_entry($name, &$value)?;)*
        map.end()
    }};
}
pub(crate) use object;

impl<'a, T> Serialize for QueryJson<'a, [T]>
where
    QueryJson<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            sequence.serialize_element(&QueryJson(value))?;
        }
        sequence.end()
    }
}

impl<'a, T> Serialize for QueryJson<'a, Option<T>>
where
    QueryJson<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(value) => serializer.serialize_some(&QueryJson(value)),
            None => serializer.serialize_none(),
        }
    }
}

mod sources;
mod types;
pub(crate) use sources::Location;
mod annotation_values;
mod constants;
mod effects;
mod requirements;
