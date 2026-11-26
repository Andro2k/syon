//! Lector de JSON mínimo para la salida de `cargo metadata`. `xtask` solo usa la biblioteca
//! estándar (ADR-0004): leer un JSON bien formado no justifica una dependencia.

/// Valor JSON.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Campo de un objeto; `None` si no es un objeto o no existe.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Interpreta un documento JSON completo.
pub fn parse(text: &str) -> Result<Json, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let value = parser.value(0)?;
    parser.skip_spaces();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("texto sobrante tras el documento"));
    }
    Ok(value)
}

/// Profundidad máxima de anidamiento; evita desbordar la pila con entradas hostiles.
const MAX_DEPTH: usize = 128;

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> String {
        format!("JSON inválido en el byte {}: {message}", self.pos)
    }

    fn skip_spaces(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("se esperaba `{}`", byte as char)))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error("valor desconocido"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("anidamiento excesivo"));
        }
        self.skip_spaces();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error("se esperaba un valor")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        self.skip_spaces();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Object(fields));
        }
        loop {
            self.skip_spaces();
            let key = self.string()?;
            self.skip_spaces();
            self.expect(b':')?;
            fields.push((key, self.value(depth + 1)?));
            self.skip_spaces();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(fields));
                }
                _ => return Err(self.error("se esperaba `,` o `}`")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_spaces();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_spaces();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.error("se esperaba `,` o `]`")),
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).expect("solo hay ASCII");
        text.parse()
            .map(Json::Number)
            .map_err(|_| self.error("número inválido"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while !matches!(self.peek(), Some(b'"' | b'\\') | None) {
                self.pos += 1;
            }
            let chunk = std::str::from_utf8(&self.bytes[start..self.pos])
                .map_err(|_| self.error("texto que no es UTF-8"))?;
            out.push_str(chunk);
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    out.push(self.escape()?);
                }
                _ => return Err(self.error("cadena sin cerrar")),
            }
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        let byte = self.peek().ok_or_else(|| self.error("escape incompleto"))?;
        self.pos += 1;
        Ok(match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => self.unicode_escape()?,
            _ => return Err(self.error("escape desconocido")),
        })
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.pos..self.pos + 4)
            .and_then(|b| std::str::from_utf8(b).ok())
            .and_then(|s| u32::from_str_radix(s, 16).ok())
            .ok_or_else(|| self.error("escape \\u inválido"))?;
        self.pos += 4;
        Ok(digits)
    }

    /// `\uXXXX`, con pares sustitutos (`\uD83D\uDE00`); un sustituto suelto da U+FFFD.
    fn unicode_escape(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        if (0xD800..0xDC00).contains(&first) && self.bytes[self.pos..].starts_with(b"\\u") {
            let saved = self.pos;
            self.pos += 2;
            let second = self.hex4()?;
            if (0xDC00..0xE000).contains(&second) {
                let code = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
                return Ok(char::from_u32(code).unwrap_or('\u{fffd}'));
            }
            self.pos = saved;
        }
        Ok(char::from_u32(first).unwrap_or('\u{fffd}'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_los_tipos_basicos() {
        assert_eq!(parse("null"), Ok(Json::Null));
        assert_eq!(parse(" true "), Ok(Json::Bool(true)));
        assert_eq!(parse("false"), Ok(Json::Bool(false)));
        assert_eq!(parse("-12.5e1"), Ok(Json::Number(-125.0)));
        assert_eq!(parse(r#""hola""#), Ok(Json::String("hola".into())));
    }

    #[test]
    fn lee_objetos_y_arreglos_anidados() {
        let json = parse(r#"{"a": [1, {"b": null}], "c": "x", "d": {}, "e": []}"#).unwrap();
        assert_eq!(json.get("c").and_then(Json::as_str), Some("x"));
        let items = json.get("a").and_then(Json::as_array).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].get("b"), Some(&Json::Null));
        assert_eq!(json.get("d"), Some(&Json::Object(vec![])));
        assert_eq!(json.get("e"), Some(&Json::Array(vec![])));
        assert_eq!(json.get("zzz"), None);
    }

    #[test]
    fn decodifica_escapes_y_unicode() {
        let json = parse(r#""a\"b\\c\/d\n\t\u00f1 \ud83d\ude00 ñ✓""#).unwrap();
        assert_eq!(json.as_str(), Some("a\"b\\c/d\n\tñ 😀 ñ✓"));
        assert_eq!(parse(r#""\ud83d""#).unwrap().as_str(), Some("\u{fffd}"));
    }

    #[test]
    fn rechaza_json_mal_formado() {
        for bad in [
            "",
            "{",
            "[1,",
            r#"{"a" 1}"#,
            r#"{"a":1,}"#,
            "tru",
            r#""sin cerrar"#,
            "1 2",
            r#""\x""#,
            r#""\u12""#,
            "[1 2]",
        ] {
            assert!(parse(bad).is_err(), "debía fallar: {bad:?}");
        }
    }

    #[test]
    fn limita_el_anidamiento() {
        let deep = "[".repeat(10_000);
        assert!(parse(&deep).unwrap_err().contains("anidamiento"));
    }
}
