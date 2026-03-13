use smallvec::SmallVec;

use crate::borrowed_value::Value;

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct GronLine<'a, 'b> {
    pub identifier: &'b [Identifier<'a>],
    pub value: GronValue<'a>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct Identifier<'a> {
    pub base: &'a str,
    pub indices: SmallVec<[Index<'a>; 2]>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone, Copy)]
pub enum Index<'a> {
    Numeric(usize),
    String(&'a str),
}

#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone, Copy)]
pub enum GronValue<'a> {
    // `json = {};`
    Object,
    // `json = [];`
    Array,
    // `json = "value";`
    String(&'a str),
    // `json = 123;`
    Number(&'a str),
    // `json = true;`
    Boolean(bool),
    // `json = null;`
    Null,
}

impl<'a> GronValue<'a> {
    pub fn to_value(self) -> Value<'a> {
        match self {
            GronValue::Object => Value::object(),
            GronValue::Array => Value::array(),
            GronValue::String(val) => Value::String(val),
            GronValue::Number(num) => Value::Number(num),
            GronValue::Boolean(b) => Value::Bool(b),
            GronValue::Null => Value::Null,
        }
    }
}
