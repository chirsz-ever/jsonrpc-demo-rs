use std::error::Error;
use std::fmt::Display;

fn escape(s: &str) -> String {
    let mut res = Vec::new();
    res.push(b'"');
    for c in s.bytes() {
        match c {
            b'"' | b'\\' => res.extend([b'\\', c]),
            b'\x08' => res.extend(b"\\b"),
            b'\x0C' => res.extend(b"\\f"),
            b'\n' => res.extend(b"\\n"),
            b'\r' => res.extend(b"\\r"),
            b'\t' => res.extend(b"\\t"),
            b'\x00'..=b'\x1F' => res.extend(format!("\\u{:04X}", c).bytes()),
            _ => res.push(c),
        }
    }
    res.push(b'"');
    String::from_utf8(res).unwrap()
}

#[derive(Debug, PartialEq, Clone)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn stringify(&self) -> String {
        match self {
            Json::Null => "null".to_string(),
            Json::Bool(b) => b.to_string(),
            Json::Number(n) => n.to_string(),
            Json::String(s) => escape(s),
            Json::Array(arr) => {
                let arr = arr
                    .iter()
                    .map(|j| j.stringify())
                    .collect::<Vec<String>>()
                    .join(",");
                format!("[{}]", arr)
            }
            Json::Object(obj) => {
                let obj = obj
                    .iter()
                    .map(|(k, v)| format!("\"{}\":{}", k, v.stringify()))
                    .collect::<Vec<String>>()
                    .join(",");
                format!("{{{}}}", obj)
            }
        }
    }

    pub fn parse_with_trailing_whitespace(s: &str) -> Result<Json> {
        let mut parser = Parser::new(s);
        parser.skip_whitespace();
        let j = parser.parse_value()?;
        parser.skip_whitespace();
        if parser.cur().is_none() {
            Ok(j)
        } else {
            parser.error("not end with trailing whitespace".into())
        }
    }
}

impl<T> From<T> for Json
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Json::String(value.into())
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub pos: usize,
    pub reason: String,
}

type Result<T> = std::result::Result<T, ParseError>;

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON Parse Error at {}: {}", self.pos, self.reason)
    }
}

impl Error for ParseError {}

fn deescape(c: u8) -> u8 {
    match c {
        b'"' => b'"',
        b'\\' => b'\\',
        b'/' => b'/',
        b'b' => b'\x08',
        b'f' => b'\x0C',
        b'n' => b'\n',
        b'r' => b'\r',
        b't' => b'\t',
        _ => unreachable!(),
    }
}

fn push_utf16(v: &mut Vec<u8>, iter: impl IntoIterator<Item = u16>) -> Result<()> {
    for c in char::decode_utf16(iter) {
        match c {
            Ok(c) => v.extend(c.encode_utf8(&mut [0; 4]).bytes()),
            Err(_e) => {
                return Err(ParseError {
                    pos: 0,
                    reason: format!("decode UTF-16 failed"),
                })
            }
        }
    }
    Ok(())
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn cur(&self) -> Option<u8> {
        self.s.as_bytes().get(self.i).copied()
    }

    fn ahead(&self, n: usize) -> Option<&str> {
        self.s.get(self.i..self.i + n)
    }

    pub fn error<T>(&self, reason: String) -> Result<T> {
        Err(ParseError {
            pos: self.i,
            reason,
        })
    }

    pub fn error_unexpected<T>(&self) -> Result<T> {
        match self.cur() {
            Some(c) => self.error(format!("unexpected char {:?}", c as char)),
            None => self.error("unexpected EOF".into()),
        }
    }

    pub fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.cur() {
            if !c.is_ascii_whitespace() {
                break;
            }
            self.i += 1;
        }
    }

    pub fn parse_value(&mut self) -> Result<Json> {
        match self.cur() {
            Some(b'n') => self.parse_identifier("null", Json::Null),
            Some(b't') => self.parse_identifier("true", Json::Bool(true)),
            Some(b'f') => self.parse_identifier("false", Json::Bool(false)),
            Some(b'0'..=b'9' | b'-') => self.parse_number(),
            Some(b'"') => Ok(Json::String(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            _ => self.error_unexpected(),
        }
    }

    fn parse_identifier(&mut self, s: &str, val: Json) -> Result<Json> {
        if self.s[self.i..].starts_with(s) {
            self.i += s.len();
            Ok(val)
        } else {
            self.error_unexpected()
        }
    }

    fn parse_number(&mut self) -> Result<Json> {
        let start = self.i;
        while let Some(c) = self.cur() {
            if !b"0123456789.-+eE".contains(&c) {
                break;
            }
            self.i += 1;
        }
        let s = &self.s[start..self.i];
        s.parse()
            .map(Json::Number)
            .or_else(|e| self.error(e.to_string()))
    }

    fn parse_string(&mut self) -> Result<String> {
        let mut v = Vec::new();
        self.i += 1;
        while let Some(c) = self.cur() {
            match c {
                b'"' => {
                    self.i += 1;
                    break;
                }
                b'\\' => {
                    self.i += 1;
                    if let Some(c) = self.cur() {
                        match c {
                            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                                v.push(deescape(c));
                            }
                            b'u' => {
                                self.i += 1;
                                let Some(hex) = self.ahead(4) else {
                                    return self.error_unexpected();
                                };
                                let Ok(n) = u16::from_str_radix(&hex, 16) else {
                                    return self.error_unexpected();
                                };
                                self.i += 4;
                                if n < 0xD800 || n > 0xDFFF {
                                    push_utf16(&mut v, [n]).or_else(|e| self.error(e.reason))?;
                                } else {
                                    // handle surrogate pair
                                    let Some("\\u") = self.ahead(2) else {
                                        return self.error_unexpected();
                                    };
                                    self.i += 2;
                                    let Some(hex1) = self.ahead(4) else {
                                        return self.error_unexpected();
                                    };
                                    let Ok(n1) = u16::from_str_radix(&hex1, 16) else {
                                        return self.error_unexpected();
                                    };
                                    self.i += 4;
                                    push_utf16(&mut v, [n, n1])?;
                                }
                            }
                            _ => return self.error_unexpected(),
                        }
                    }
                }
                _ => {
                    v.push(c);
                    self.i += 1;
                }
            }
        }
        Ok(String::from_utf8(v).unwrap())
    }

    // '[' ']' | '[' value (',' value)* ']'
    fn parse_array(&mut self) -> Result<Json> {
        let mut arr = Vec::new();
        self.i += 1;
        let mut first = true;
        while let Some(_) = self.cur() {
            self.skip_whitespace();
            if let Some(b']') = self.cur() {
                self.i += 1;
                return Ok(Json::Array(arr));
            }
            if !first {
                if let Some(b',') = self.cur() {
                    self.i += 1;
                } else {
                    return self.error_unexpected();
                }
                self.skip_whitespace();
            }
            arr.push(self.parse_value()?);
            first = false;
        }
        self.error_unexpected()
    }

    // '{' '}' | '{' key ':' value (',' key ':' value)* '}'
    fn parse_object(&mut self) -> Result<Json> {
        let mut obj = Vec::new();
        self.i += 1;
        let mut first = true;
        while let Some(_) = self.cur() {
            self.skip_whitespace();
            if let Some(b'}') = self.cur() {
                self.i += 1;
                return Ok(Json::Object(obj));
            }
            if !first {
                if let Some(b',') = self.cur() {
                    self.i += 1;
                } else {
                    return self.error_unexpected();
                }
                self.skip_whitespace();
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            let Some(b':') = self.cur() else {
                return self.error_unexpected();
            };
            self.i += 1;
            self.skip_whitespace();
            let val = self.parse_value()?;
            obj.push((key, val));
            first = false;
        }
        self.error_unexpected()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_escape() {
        assert_eq!(escape(""), r#""""#);
        assert_eq!(escape("a"), r#""a""#);
        assert_eq!(escape("a\0"), r#""a\u0000""#);
        assert_eq!(escape("a\x00"), r#""a\u0000""#);
        assert_eq!(escape("a\x01"), r#""a\u0001""#);
        assert_eq!(escape("a\x1F"), r#""a\u001F""#);
        assert_eq!(escape("a\x20"), r#""a ""#);
        assert_eq!(escape("a\x7E"), r#""a~""#);
        assert_eq!(escape("你好"), r#""你好""#);
        assert_eq!(escape("a\""), r#""a\"""#);
        assert_eq!(escape("a\\"), r#""a\\""#);
    }

    #[test]
    fn test_stringify() {
        use Json::*;
        assert_stringify_eq(Null, "null");
        assert_stringify_eq(Bool(true), "true");
        assert_stringify_eq(Bool(false), "false");
        assert_stringify_eq(Number(1.), "1");
        assert_stringify_eq(Number(0.), "0");
        assert_stringify_eq(Number(-1.), "-1");
        assert_stringify_eq(Number(1.5), "1.5");
        assert_stringify_eq(String("abc".into()), "\"abc\"");
        assert_stringify_eq(Array(vec![]), "[]");
        assert_stringify_eq(Array(vec![Null, Bool(false)]), "[null,false]");
        assert_stringify_eq(Array(vec![Array(vec![]), Bool(false)]), "[[],false]");
        assert_stringify_eq(Object(vec![]), "{}");
        assert_stringify_eq(Object(vec![("x".into(), Number(1.))]), "{\"x\":1}");
        assert_stringify_eq(
            Object(vec![("x".into(), Number(1.)), ("y".into(), Null)]),
            "{\"x\":1,\"y\":null}",
        );
    }

    fn assert_stringify_eq(j: Json, s: &str) {
        assert_eq!(j.stringify(), s);
    }

    #[test]
    fn test_parse() {
        use Json::*;
        assert_parse_eq(Null, "null");
        assert_parse_eq(Bool(true), "true");
        assert_parse_eq(Bool(false), "false");
        assert_parse_eq(Number(1.), "1");
        assert_parse_eq(Number(0.), "0");
        assert_parse_eq(Number(-1.), "-1");
        assert_parse_eq(Number(1.5), "1.5");
        assert_parse_eq(String("abc".into()), "\"abc\"");
        assert_parse_eq(Array(vec![]), "[]");
        assert_parse_eq(Array(vec![Null, Bool(false)]), "[null,false]");
        assert_parse_eq(Array(vec![Array(vec![]), Bool(false)]), "[[],false]");
        assert_parse_eq(Object(vec![]), "{}");
        assert_parse_eq(Object(vec![("x".into(), Number(1.))]), "{\"x\":1}");
        assert_parse_eq(
            Object(vec![("x".into(), Number(1.)), ("y".into(), Null)]),
            "{\"x\":1,\"y\":null}",
        );
    }

    fn assert_parse_eq(j: Json, s: &str) {
        assert_eq!(j, Json::parse_with_trailing_whitespace(s).unwrap());
    }
}
