use indexmap::IndexMap;
use jsax::Event;
use rapidhash::fast::RandomState;

pub type Map<'a> = IndexMap<&'a str, Value<'a>, RandomState>;

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

pub fn parse_value(text: &str) -> Result<Value<'_>, jsax::Error> {
    let mut parser = jsax::Parser::new(text);
    let mut stack = Vec::new();
    let mut key_stack = Vec::new();

    while let Some(event) = parser.parse_next()? {
        match event {
            Event::StartObject => {
                stack.push(Value::object());
                key_stack.push(None);
            }

            Event::EndObject { .. } => {
                let completed = stack.pop().expect("stack underflow on EndObject");
                key_stack.pop();

                if stack.is_empty() {
                    return Ok(completed);
                }

                let current_key = key_stack.last_mut().unwrap();
                add_to_parent_with_key(&mut stack, current_key, completed);
            }

            Event::StartArray => {
                stack.push(Value::array());
                key_stack.push(None);
            }

            Event::EndArray { .. } => {
                let completed = stack.pop().expect("stack underflow on EndArray");
                key_stack.pop();

                if stack.is_empty() {
                    return Ok(completed);
                }

                let current_key = key_stack.last_mut().unwrap();
                add_to_parent_with_key(&mut stack, current_key, completed);
            }

            Event::Key(key) => {
                *key_stack.last_mut().unwrap() = Some(key);
            }

            Event::Null => {
                let value = Value::Null;
                if stack.is_empty() {
                    return Ok(value);
                }
                let current_key = key_stack.last_mut().unwrap();
                add_to_parent_with_key(&mut stack, current_key, value);
            }

            Event::Boolean(b) => {
                let value = Value::Bool(b);
                if stack.is_empty() {
                    return Ok(value);
                }
                let current_key = key_stack.last_mut().unwrap();
                add_to_parent_with_key(&mut stack, current_key, value);
            }

            Event::Number(n) => {
                let value = Value::Number(n);
                if stack.is_empty() {
                    return Ok(value);
                }
                let current_key = key_stack.last_mut().unwrap();
                add_to_parent_with_key(&mut stack, current_key, value);
            }

            Event::String(s) => {
                let value = Value::String(s);
                if stack.is_empty() {
                    return Ok(value);
                }
                let current_key = key_stack.last_mut().unwrap();
                add_to_parent_with_key(&mut stack, current_key, value);
            }
        }
    }

    Err(jsax::Error::Unexpected(
        "empty or incomplete JSON".to_string(),
    ))
}

fn add_to_parent_with_key<'a>(
    stack: &mut Vec<Value<'a>>,
    current_key: &mut Option<&'a str>,
    value: Value<'a>,
) {
    let parent = stack.last_mut().expect("parent container missing");

    match parent {
        Value::Object(map) => {
            let key = current_key.take().expect("key missing for object entry");
            map.insert(key, value);
        }
        Value::Array(arr) => {
            arr.push(value);
        }
        _ => panic!("parent must be Object or Array"),
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
    use jsax::Parser;

    use super::*;
    use crate::{Formatter, unflatten::unflatten_to_value};

    fn format_value(value: &Value<'_>) -> String {
        let mut output = Vec::new();
        Formatter::new_plain(ValueEvents::new(value))
            .format_to(&mut output)
            .unwrap();
        String::from_utf8(output).unwrap()
    }

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

    #[test]
    fn test_parse_primitives() {
        let null = parse_value("null").unwrap();
        assert!(matches!(null, Value::Null));

        let bool_true = parse_value("true").unwrap();
        assert!(matches!(bool_true, Value::Bool(true)));

        let bool_false = parse_value("false").unwrap();
        assert!(matches!(bool_false, Value::Bool(false)));

        let num = parse_value("42").unwrap();
        assert!(matches!(num, Value::Number("42")));

        let string = parse_value(r#""hello""#).unwrap();
        assert!(matches!(string, Value::String("hello")));
    }

    #[test]
    fn test_parse_empty_containers() {
        let empty_obj = parse_value("{}").unwrap();
        match empty_obj {
            Value::Object(map) => assert_eq!(map.len(), 0),
            _ => panic!("expected Object"),
        }

        let empty_arr = parse_value("[]").unwrap();
        match empty_arr {
            Value::Array(arr) => assert_eq!(arr.len(), 0),
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn test_parse_simple_object() {
        let json = r#"{"name": "Alice", "age": 30, "active": true}"#;
        let value = parse_value(json).unwrap();

        match value {
            Value::Object(map) => {
                assert_eq!(map.len(), 3);
                assert!(matches!(map.get("name"), Some(Value::String("Alice"))));
                assert!(matches!(map.get("age"), Some(Value::Number("30"))));
                assert!(matches!(map.get("active"), Some(Value::Bool(true))));
            }
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn test_parse_twitter_json() {
        fn format_str(json: &str) -> String {
            let mut output = Vec::new();
            Formatter::new_plain(Parser::new(json))
                .format_to(&mut output)
                .unwrap();
            String::from_utf8(output).unwrap()
        }

        let json = include_str!("../samples/twitter.json");
        let value = parse_value(json).unwrap();
        let formatted_from_value = format_value(&value);
        let formatted_from_str = format_str(json);
        assert_eq!(formatted_from_value, formatted_from_str);
    }

    #[test]
    fn test_parse_simple_array() {
        let json = r#"[1, "two", true, null]"#;
        let value = parse_value(json).unwrap();

        match value {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 4);
                assert!(matches!(arr[0], Value::Number("1")));
                assert!(matches!(arr[1], Value::String("two")));
                assert!(matches!(arr[2], Value::Bool(true)));
                assert!(matches!(arr[3], Value::Null));
            }
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn test_parse_nested_structures() {
        let json = r#"{"users": [{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]}"#;
        let value = parse_value(json).unwrap();

        match value {
            Value::Object(map) => match map.get("users") {
                Some(Value::Array(users)) => {
                    assert_eq!(users.len(), 2);
                    match &users[0] {
                        Value::Object(user) => {
                            assert!(matches!(user.get("name"), Some(Value::String("Alice"))));
                            assert!(matches!(user.get("age"), Some(Value::Number("30"))));
                        }
                        _ => panic!("expected Object in array"),
                    }
                }
                _ => panic!("expected Array for users"),
            },
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn test_parse_nested() {
        let json = r#"{"a": {"b": {"c": {"d": "deep"}}}}"#;
        let value = parse_value(json).unwrap();

        if let Value::Object(a) = value
            && let Some(Value::Object(b)) = a.get("a")
            && let Some(Value::Object(c)) = b.get("b")
            && let Some(Value::Object(d)) = c.get("c")
        {
            assert!(matches!(d.get("d"), Some(Value::String("deep"))));
            return;
        }
        panic!("failed to traverse nested structure");
    }

    #[test]
    fn test_parse_array_of_arrays() {
        let json = r#"[[1, 2], [3, 4], [5, 6]]"#;
        let value = parse_value(json).unwrap();

        match value {
            Value::Array(outer) => {
                assert_eq!(outer.len(), 3);
                for arr in outer {
                    match arr {
                        Value::Array(inner) => assert_eq!(inner.len(), 2),
                        _ => panic!("expected inner Array"),
                    }
                }
            }
            _ => panic!("expected Array"),
        }
    }
}
