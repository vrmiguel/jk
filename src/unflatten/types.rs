#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct GronLine<'a> {
    pub identifier: Vec<Identifier<'a>>,
    pub value: GronValue<'a>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct Identifier<'a> {
    pub base: &'a str,
    // TODO: this has to be `Vec<Index<'a>>`
    pub index: Option<Index<'a>>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone, Copy)]
pub enum Index<'a> {
    Numeric(&'a str),
    String(&'a str),
}

#[cfg_attr(test, derive(PartialEq, Debug))]
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

impl GronValue<'_> {
    pub fn to_serde(&self) -> serde_json::Value {
        match self {
            GronValue::Object => serde_json::Value::Object(serde_json::Map::new()),
            GronValue::Array => serde_json::Value::Array(Vec::new()),
            GronValue::String(val) => serde_json::Value::String(val.to_string()),
            GronValue::Number(num) => {
                let num: f64 = num.parse().unwrap();
                let num = serde_json::Number::from_f64(num).unwrap();
                serde_json::Value::Number(num)
            }
            GronValue::Boolean(b) => serde_json::Value::Bool(*b),
            GronValue::Null => serde_json::Value::Null,
        }
    }
}
