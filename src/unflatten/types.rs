#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct GronLine<'a> {
    pub identifier: Vec<Identifier<'a>>,
    pub value: GronValue<'a>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct Identifier<'a> {
    pub base: &'a str,
    pub index: Option<Index<'a>>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
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
