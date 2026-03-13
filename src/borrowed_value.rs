use indexmap::IndexMap;
use jsax::Event;
use rapidhash::fast::RandomState as FastHash;

pub type Map<'a> = IndexMap<&'a str, Value<'a>, FastHash>;

#[derive(Clone, Debug)]
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
        Self::Object(IndexMap::default())
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

/// ValueEvents is basically an iterator through a [`Value`], useful
/// to be passed over to the [`Formatter`]
pub struct ValueEvents<'a> {
    stack: Vec<Step<'a>>,
}

enum Step<'a> {
    /// An event ready to be emitted
    Emit(Event<'a>),
    /// A cursor lazily going through an object's entries
    InObject { map: &'a Map<'a>, index: usize },
    /// A cursor lazily iterating through an array's elements
    InArray { remaining: &'a [Value<'a>] },
}

impl<'a> ValueEvents<'a> {
    pub fn new(value: &'a Value<'a>) -> Self {
        let mut this = Self {
            stack: Vec::with_capacity(8),
        };
        this.push_value(value);
        this
    }

    fn push_value(&mut self, value: &'a Value<'a>) {
        match value {
            Value::Null => self.stack.push(Step::Emit(Event::Null)),
            Value::Bool(b) => self.stack.push(Step::Emit(Event::Boolean(*b))),
            Value::Number(n) => self.stack.push(Step::Emit(Event::Number(n))),
            Value::String(s) => self.stack.push(Step::Emit(Event::String(s))),
            Value::Object(map) => {
                self.stack.push(Step::Emit(Event::EndObject {
                    member_count: map.len(),
                }));
                self.stack.push(Step::InObject { map, index: 0 });
                self.stack.push(Step::Emit(Event::StartObject));
            }
            Value::Array(arr) => {
                self.stack
                    .push(Step::Emit(Event::EndArray { len: arr.len() }));
                self.stack.push(Step::InArray { remaining: arr });
                self.stack.push(Step::Emit(Event::StartArray));
            }
        }
    }
}

impl<'a> Iterator for ValueEvents<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.stack.pop()? {
                Step::Emit(event) => return Some(event),
                Step::InObject { map, index } => {
                    if let Some((key, val)) = map.get_index(index) {
                        self.stack.push(Step::InObject {
                            map,
                            index: index + 1,
                        });
                        self.push_value(val);
                        return Some(Event::Key(key));
                    }
                    // Continues, next iteration pops Emit(EndObject)
                }
                Step::InArray { remaining } => {
                    if let Some((val, rest)) = remaining.split_first() {
                        self.stack.push(Step::InArray { remaining: rest });
                        self.push_value(val);
                    }
                    // Continues, next iteration pops Emit(EndArray) or emits pushed value
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
