use indexmap::IndexMap;
use rapidhash::fast::RandomState;
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

pub type Map<'a> = IndexMap<&'a str, Value<'a>, RandomState>;

#[derive(Clone)]
pub enum Value<'a> {
    Null,
    Bool(bool),
    Number(&'a str),
    String(&'a str),
    Array(Vec<Value<'a>>),
    Object(Map<'a>),
}

impl<'a> Serialize for Value<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Number(n) => {
                if let Ok(i) = n.parse::<i64>() {
                    serializer.serialize_i64(i)
                } else if let Ok(u) = n.parse::<u64>() {
                    serializer.serialize_u64(u)
                } else if let Ok(f) = n.parse::<f64>() {
                    serializer.serialize_f64(f)
                } else {
                    serializer.serialize_str(n)
                }
            }
            Value::String(s) => serializer.serialize_str(s),
            Value::Array(arr) => {
                let mut seq = serializer.serialize_seq(Some(arr.len()))?;
                for element in arr {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            Value::Object(map) => {
                let mut map_ser = serializer.serialize_map(Some(map.len()))?;
                for (key, value) in map {
                    map_ser.serialize_entry(key, value)?;
                }
                map_ser.end()
            }
        }
    }
}

impl<'a> Value<'a> {
    pub fn object() -> Self {
        Self::Object(IndexMap::with_hasher(RandomState::default()))
    }

    pub fn array() -> Self {
        Self::Array(Vec::new())
    }

    pub fn as_object(&self) -> Option<&Map<'_>> {
        match self {
            Value::Object(inner) => Some(inner),
            _ => None,
        }
    }
    pub fn as_object_mut(&mut self) -> Option<&mut Map<'a>> {
        match self {
            Value::Object(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value<'a>>> {
        match self {
            Value::Array(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value<'a>>> {
        match self {
            Value::Array(inner) => Some(inner),
            _ => None,
        }
    }
}
