use indexmap::IndexMap;
use jsax::Event;
use rapidhash::fast::RandomState;

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

impl<'a> Value<'a> {
    pub fn object() -> Self {
        Self::Object(IndexMap::with_hasher(RandomState::default()))
    }

    pub fn array() -> Self {
        Self::Array(Vec::new())
    }

    pub fn as_object_mut(&mut self) -> Option<&mut Map<'a>> {
        match self {
            Value::Object(inner) => Some(inner),
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

pub struct ValueEvents<'a> {
    stack: Vec<State<'a>>,
}

enum State<'a> {
    EmitValue(&'a Value<'a>),
    InObject {
        map: &'a Map<'a>,
        index: usize,
    },
    InArray {
        array: &'a Vec<Value<'a>>,
        index: usize,
    },
    EmitEndObject {
        member_count: usize,
    },
    EmitEndArray {
        len: usize,
    },
}

impl<'a> ValueEvents<'a> {
    pub fn new(value: &'a Value<'a>) -> Self {
        Self {
            stack: vec![State::EmitValue(value)],
        }
    }
}

impl<'a> Iterator for ValueEvents<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let state = self.stack.pop()?;

            match state {
                State::EmitValue(value) => {
                    return Some(match value {
                        Value::Null => Event::Null,
                        Value::Bool(b) => Event::Boolean(*b),
                        Value::Number(n) => Event::Number(n),
                        Value::String(s) => Event::String(s),
                        Value::Object(map) => {
                            let member_count = map.len();
                            self.stack.push(State::EmitEndObject { member_count });
                            self.stack.push(State::InObject { map, index: 0 });
                            Event::StartObject
                        }
                        Value::Array(arr) => {
                            let len = arr.len();
                            self.stack.push(State::EmitEndArray { len });
                            self.stack.push(State::InArray {
                                array: arr,
                                index: 0,
                            });
                            Event::StartArray
                        }
                    });
                }

                State::InObject { map, index } => {
                    if let Some((key, value)) = map.get_index(index) {
                        self.stack.push(State::InObject {
                            map,
                            index: index + 1,
                        });
                        self.stack.push(State::EmitValue(value));
                        return Some(Event::Key(key));
                    }
                    // Will continue to pop EndObject
                }

                State::InArray { array, index } => {
                    if let Some(value) = array.get(index) {
                        self.stack.push(State::InArray {
                            array,
                            index: index + 1,
                        });
                        self.stack.push(State::EmitValue(value));
                        // Will continue to emit the value
                    }
                    // Will continue to pop EndArray
                }

                State::EmitEndObject { member_count } => {
                    return Some(Event::EndObject { member_count });
                }

                State::EmitEndArray { len } => {
                    return Some(Event::EndArray { len });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unflatten::unflatten_to_value;

    #[test]
    fn test_value_events() {
        let input = r#"json = {};
json.hobbies = [];
json.hobbies[0] = [];
json.hobbies[0][0] = "reading";
json.hobbies[0][1] = "cycling";
json.hobbies[1] = [];
json.hobbies[1][0] = "swimming";
json.hobbies[1][1] = "dancing";"#;

        // Using our unflatten impl here since that's faster than manually building the value
        let value = unflatten_to_value(input).unwrap();
        let events = ValueEvents::new(&value);
        let events = events.collect::<Vec<_>>();
        // assert_eq!(events.len(), 1);
        dbg!(events);
    }
}
