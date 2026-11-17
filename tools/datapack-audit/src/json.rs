//! Minimal strict JSON parser (RFC 8259). Mirrors serde_json's strictness on the
//! points that matter for datapacks: no comments, no trailing commas, no BOM,
//! duplicate object keys resolve to the last value.

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(m) => m.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str_of(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Value]> {
        match self {
            Value::Arr(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Num(n) if n.fract() == 0.0 => Some(*n as i64),
            _ => None,
        }
    }
    pub fn is_obj(&self) -> bool {
        matches!(self, Value::Obj(_))
    }
    pub fn is_num(&self) -> bool {
        matches!(self, Value::Num(_))
    }
    /// Array elements, or the value itself as a one-element slice.
    pub fn many(&self) -> &[Value] {
        match self {
            Value::Arr(a) => a,
            other => std::slice::from_ref(other),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub offset: usize,
    pub msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.msg, self.offset)
    }
}

const MAX_DEPTH: usize = 512;

pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut p = Parser { b: text.as_bytes(), i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() {
        return p.err("trailing characters");
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err<T>(&self, msg: &str) -> Result<T, ParseError> {
        Err(ParseError { offset: self.i, msg: msg.to_string() })
    }
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn lit(&mut self, s: &str, v: Value) -> Result<Value, ParseError> {
        if self.b[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            Ok(v)
        } else {
            self.err("expected value")
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > MAX_DEPTH {
            return self.err("recursion limit exceeded");
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.lit("true", Value::Bool(true)),
            Some(b'f') => self.lit("false", Value::Bool(false)),
            Some(b'n') => self.lit("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => self.err("expected value"),
            None => self.err("EOF while parsing a value"),
        }
    }
    fn object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.i += 1;
        let mut m = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Obj(m));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return self.err("key must be a string");
            }
            let k = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return self.err("expected ':'");
            }
            self.i += 1;
            self.ws();
            let v = self.value(depth + 1)?;
            m.push((k, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Obj(m));
                }
                _ => return self.err("expected ',' or '}'"),
            }
        }
    }
    fn array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.i += 1;
        let mut a = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Arr(a));
        }
        loop {
            self.ws();
            a.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Arr(a));
                }
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, ParseError> {
        if self.i + 4 > self.b.len() {
            return self.err("EOF in \\u escape");
        }
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4]).map_err(|_| ParseError { offset: self.i, msg: "bad \\u".into() })?;
        let v = u32::from_str_radix(s, 16).map_err(|_| ParseError { offset: self.i, msg: "bad \\u escape".into() })?;
        self.i += 4;
        Ok(v)
    }
    fn string(&mut self) -> Result<String, ParseError> {
        self.i += 1;
        let mut out = String::new();
        let start = self.i;
        let mut run_start = start;
        loop {
            let Some(c) = self.peek() else { return self.err("EOF while parsing a string") };
            match c {
                b'"' => {
                    out.push_str(std::str::from_utf8(&self.b[run_start..self.i]).unwrap_or_default());
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    out.push_str(std::str::from_utf8(&self.b[run_start..self.i]).unwrap_or_default());
                    self.i += 1;
                    let Some(e) = self.peek() else { return self.err("EOF in escape") };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                if self.b[self.i..].starts_with(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return self.err("lone leading surrogate");
                                    }
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                } else {
                                    return self.err("lone leading surrogate");
                                }
                            }
                            match char::from_u32(cp) {
                                Some(ch) => out.push(ch),
                                None => return self.err("invalid unicode code point"),
                            }
                        }
                        _ => return self.err("invalid escape"),
                    }
                    run_start = self.i;
                }
                0..=0x1F => return self.err("control character in string"),
                _ => self.i += 1,
            }
        }
    }
    fn number(&mut self) -> Result<Value, ParseError> {
        let s = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return self.err("invalid number"),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.err("invalid number");
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.err("invalid number");
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let t = std::str::from_utf8(&self.b[s..self.i]).unwrap_or("0");
        t.parse::<f64>().map(Value::Num).or_else(|_| self.err("invalid number"))
    }
}

/// Serialise a string as a JSON string literal.
pub fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_and_escapes() {
        let v = parse(r#"{"a":[1,-2.5e3,true,null],"b":"xé😀\n","a":7}"#).unwrap();
        assert_eq!(v.get("a"), Some(&Value::Num(7.0)), "duplicate key: last wins like serde_json");
        assert_eq!(v.str_of("b"), Some("xé😀\n"));
    }

    #[test]
    fn rejects_what_serde_json_rejects() {
        for bad in ["{\"a\":1,}", "[1,]", "\u{feff}{}", "{a:1}", "// c\n{}", "{\"a\":01}", "[1] x", "\"\t\""] {
            assert!(parse(bad).is_err(), "should reject {bad:?}");
        }
    }
}
