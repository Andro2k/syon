use std::{borrow::Cow, path::Path};

/// Texto que sustituye a cada secreto eliminado.
const MASK: &str = "[REDACTADO]";
/// Texto que sustituye a la carpeta personal.
const HOME_MASK: &str = "~";
/// Un `code` más corto (`code=404`) es un estado, no un código de autorización de OAuth.
const MIN_AUTH_CODE_LEN: usize = 8;
/// Claves cuyo nombre termina así guardan un secreto (`access_token`, `clientSecret`...).
const SECRET_SUFFIXES: [&str; 8] = [
    "token", "secret", "password", "passwd", "cookie", "verifier", "api_key", "api-key",
];

/// Quita de una línea de registro lo que no debe salir del equipo: valores de claves secretas
/// (tokens, secretos, contraseñas, códigos de OAuth), credenciales `Bearer` y la carpeta personal.
///
/// Es una red de seguridad: reconoce secretos por el nombre de su clave y no puede adivinar uno
/// suelto. La regla sigue siendo no registrarlos (AGENTS.md).
pub(crate) struct Redactor {
    home_forms: Vec<String>,
}

impl Redactor {
    /// Crea un redactor que además oculta `home`. Una carpeta personal que sea la raíz del
    /// sistema se ignora: sustituirla estropearía todas las rutas.
    pub(crate) fn new(home: Option<&Path>) -> Self {
        let mut home_forms = Vec::new();
        if let Some(home) = home.filter(|home| home.parent().is_some()) {
            let text = home.to_string_lossy().into_owned();
            // La misma carpeta aparece con `/` y con `\\` (la escapa `{:?}` en Windows).
            for form in [
                text.replace('\\', "/"),
                text.replace('\\', "\\\\"),
                text.clone(),
            ] {
                if form.len() >= 3 && !home_forms.contains(&form) {
                    home_forms.push(form);
                }
            }
        }
        Self { home_forms }
    }

    /// Devuelve la línea sin secretos ni carpeta personal; no copia nada si no había ninguno.
    pub(crate) fn redact<'a>(&self, line: &'a str) -> Cow<'a, str> {
        let mut text = redact_secrets(line);
        for form in &self.home_forms {
            if let Some(replaced) = replace_home(&text, form) {
                text = Cow::Owned(replaced);
            }
        }
        text
    }
}

/// Sustituye el valor de cada clave secreta y cada credencial `Bearer`.
fn redact_secrets(line: &str) -> Cow<'_, str> {
    let bytes = line.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut changed = false;
    let mut i = 0;
    while i < bytes.len() {
        if !is_word_byte(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && is_word_byte(bytes[i]) {
            i += 1;
        }
        if let Some((from, to)) = secret_span(line, &line[start..i], i) {
            out.push_str(&line[copied..from]);
            out.push_str(MASK);
            copied = to;
            i = to;
            changed = true;
        }
    }
    if changed {
        out.push_str(&line[copied..]);
        Cow::Owned(out)
    } else {
        Cow::Borrowed(line)
    }
}

/// Rango del secreto que sigue a la palabra `word` (que termina en `after`), si lo hay.
fn secret_span(line: &str, word: &str, after: usize) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    if word.eq_ignore_ascii_case("bearer") {
        let gap = count_while(bytes, after, is_blank);
        // `bearer expires_in=3600` no es una credencial: es el valor de `token_type` y otra clave.
        return value_span(bytes, after + gap)
            .filter(|&(from, to)| gap > 0 && !line[from..to].trim_end_matches('=').contains('='));
    }
    if !is_secret_key(word) {
        return None;
    }
    // Entre la clave y el valor puede haber comillas (JSON, `{:?}`), `=` o `:` y espacios.
    let mut pos = after + count_while(bytes, after, is_quote);
    pos += count_while(bytes, pos, is_blank);
    if !matches!(bytes.get(pos), Some(b'=' | b':')) {
        return None;
    }
    pos += 1;
    pos += count_while(bytes, pos, is_blank);
    pos += count_while(bytes, pos, is_quote);
    let (from, to) = value_span(bytes, pos)?;
    let value = &line[from..to];
    let short_code = word.eq_ignore_ascii_case("code") && value.len() < MIN_AUTH_CODE_LEN;
    // `Authorization: Bearer xyz`: el secreto es `xyz`, que se trata al llegar a `Bearer`.
    (!short_code && !value.eq_ignore_ascii_case("bearer")).then_some((from, to))
}

/// Rango no vacío del valor que empieza en `from` y llega hasta un separador.
fn value_span(bytes: &[u8], from: usize) -> Option<(usize, usize)> {
    let len = count_while(bytes, from, |b| !is_value_end(b));
    (len > 0).then_some((from, from + len))
}

fn is_secret_key(word: &str) -> bool {
    let word = word.to_ascii_lowercase();
    word == "code"
        || word == "authorization"
        || SECRET_SUFFIXES.iter().any(|suffix| word.ends_with(suffix))
}

/// Sustituye cada aparición de `home` que sea una ruta completa (no `/home/luis2` si `home` es
/// `/home/luis`). Devuelve `None` si no había ninguna.
fn replace_home(text: &str, home: &str) -> Option<String> {
    let mut out = String::new();
    let mut copied = 0;
    let mut search = 0;
    while let Some(found) = text[search..].find(home) {
        let from = search + found;
        let to = from + home.len();
        search = to;
        let before_ok = text[..from]
            .bytes()
            .next_back()
            .is_none_or(|b| !is_path_name_byte(b));
        let after_ok = text
            .as_bytes()
            .get(to)
            .is_none_or(|&b| !is_path_name_byte(b));
        if before_ok && after_ok {
            out.push_str(&text[copied..from]);
            out.push_str(HOME_MASK);
            copied = to;
        }
    }
    (copied > 0).then(|| {
        out.push_str(&text[copied..]);
        out
    })
}

fn count_while(bytes: &[u8], from: usize, keep: impl Fn(u8) -> bool) -> usize {
    bytes[from.min(bytes.len())..]
        .iter()
        .take_while(|&&b| keep(b))
        .count()
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_quote(b: u8) -> bool {
    matches!(b, b'"' | b'\'' | b'\\')
}

fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Donde termina un valor: espacio o un signo que lo cierra (`&` de una URL, comillas, `}`...).
fn is_value_end(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b'&' | b',' | b';' | b')' | b']' | b'}') || is_quote(b)
}

/// Byte que puede formar parte del nombre de una carpeta, para no cortar `/home/luis2`.
fn is_path_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'~')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::MAIN_SEPARATOR;

    fn redact(line: &str) -> String {
        Redactor::new(None).redact(line).into_owned()
    }

    fn redact_with_home(line: &str, home: &str) -> String {
        Redactor::new(Some(Path::new(home)))
            .redact(line)
            .into_owned()
    }

    #[test]
    fn una_linea_sin_secretos_no_se_copia() {
        let line = "bus: consumidor 'tts' saturado, 3 descartados";
        assert!(matches!(Redactor::new(None).redact(line), Cow::Borrowed(_)));
    }

    #[test]
    fn oculta_el_valor_de_las_claves_con_igual_o_dos_puntos() {
        assert_eq!(
            redact("kick: access_token=abc123 client_secret: s3cr3t"),
            "kick: access_token=[REDACTADO] client_secret: [REDACTADO]"
        );
    }

    #[test]
    fn oculta_los_valores_de_un_json() {
        assert_eq!(
            redact(r#"{"access_token":"abc","refresh_token":"def","expires_in":3600}"#),
            r#"{"access_token":"[REDACTADO]","refresh_token":"[REDACTADO]","expires_in":3600}"#
        );
    }

    #[test]
    fn oculta_los_valores_en_el_formato_debug_y_con_comillas_escapadas() {
        assert_eq!(
            redact(r#"tokens: Tokens { access_token: "abc", scope: "chat" }"#),
            r#"tokens: Tokens { access_token: "[REDACTADO]", scope: "chat" }"#
        );
        assert_eq!(
            redact(r#"respuesta: "{\"refresh_token\":\"def\"}""#),
            r#"respuesta: "{\"refresh_token\":\"[REDACTADO]\"}""#
        );
    }

    #[test]
    fn oculta_el_codigo_de_oauth_de_una_url_pero_no_un_estado_http() {
        assert_eq!(
            redact("GET /cb?code=ab12cd34ef&state=xyz"),
            "GET /cb?code=[REDACTADO]&state=xyz"
        );
        assert_eq!(redact("respuesta code=404"), "respuesta code=404");
    }

    #[test]
    fn oculta_el_verificador_pkce_y_la_contrasena() {
        assert_eq!(
            redact("code_verifier=v3r1f password=hunter2"),
            "code_verifier=[REDACTADO] password=[REDACTADO]"
        );
    }

    #[test]
    fn oculta_la_credencial_bearer() {
        assert_eq!(
            redact("Authorization: Bearer abc.def-ghi"),
            "Authorization: Bearer [REDACTADO]"
        );
        assert_eq!(redact("cabecera bearer abc"), "cabecera bearer [REDACTADO]");
    }

    #[test]
    fn no_distingue_mayusculas_ni_estilo_de_la_clave() {
        assert_eq!(
            redact("ACCESS_TOKEN=a clientSecret=b"),
            "ACCESS_TOKEN=[REDACTADO] clientSecret=[REDACTADO]"
        );
    }

    #[test]
    fn deja_intactas_las_claves_que_no_guardan_secretos() {
        for line in [
            "token_type=bearer expires_in=3600",
            "max_tokens=10",
            "client_id=abc123",
            "el token venció",
            "token:",
        ] {
            assert_eq!(redact(line), line);
        }
    }

    #[test]
    fn conserva_el_texto_no_ascii_alrededor_del_secreto() {
        assert_eq!(
            redact("ñandú token=abc, canción ñ"),
            "ñandú token=[REDACTADO], canción ñ"
        );
    }

    #[test]
    fn oculta_la_carpeta_personal_y_conserva_el_resto_de_la_ruta() {
        assert_eq!(
            redact_with_home(
                "ajustes: leyendo /home/luis/.config/syon/settings.toml",
                "/home/luis"
            ),
            "ajustes: leyendo ~/.config/syon/settings.toml"
        );
    }

    #[test]
    fn no_corta_carpetas_que_solo_empiezan_igual() {
        for line in ["/home/luis2/x", "/mnt/home/luis/x", "/home/luis.bak"] {
            assert_eq!(redact_with_home(line, "/home/luis"), line);
        }
    }

    #[test]
    fn oculta_la_carpeta_de_windows_con_sus_tres_escrituras() {
        let home = r"C:\Users\luis";
        assert_eq!(
            redact_with_home(r"C:\Users\luis\AppData\x", home),
            r"~\AppData\x"
        );
        assert_eq!(redact_with_home(r"C:\\Users\\luis\\x", home), r"~\\x");
        assert_eq!(redact_with_home("C:/Users/luis/x", home), "~/x");
    }

    #[test]
    fn una_carpeta_personal_que_es_la_raiz_se_ignora() {
        let root = MAIN_SEPARATOR.to_string();
        assert_eq!(redact_with_home("/etc/hosts", &root), "/etc/hosts");
    }

    #[test]
    fn oculta_secretos_y_carpeta_en_la_misma_linea() {
        assert_eq!(
            redact_with_home("/home/luis/tokens.json token=abc", "/home/luis"),
            "~/tokens.json token=[REDACTADO]"
        );
    }
}
