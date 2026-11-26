use crate::error::OAuthError;
use reqwest::Url;
use std::collections::HashMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Qué significa una petición recibida en la dirección de callback.
#[derive(Debug, PartialEq)]
pub(crate) enum Callback {
    /// Autorización concedida, con su `code`.
    Code(String),
    /// Twitch rechazó la autorización o la respuesta no es válida.
    Failed(String),
    /// No es la petición esperada (por ejemplo, el `favicon.ico`).
    Unrelated,
}

/// Interpreta la primera línea de una petición HTTP. Solo cuenta un `GET` a `expected_path` con el
/// `state` esperado, que protege contra CSRF.
pub(crate) fn parse_callback(request: &str, expected_path: &str, expected_state: &str) -> Callback {
    let target = request.lines().next().and_then(|line| {
        let mut parts = line.split_whitespace();
        (parts.next() == Some("GET"))
            .then(|| parts.next())
            .flatten()
    });
    let Some(url) = target.and_then(|t| Url::parse("http://localhost").ok()?.join(t).ok()) else {
        return Callback::Unrelated;
    };
    if url.path() != expected_path {
        return Callback::Unrelated;
    }
    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(error) = params.get("error") {
        let detail = params.get("error_description").map_or("", String::as_str);
        return Callback::Failed(format!("Twitch rechazó la autorización: {error} {detail}"));
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Callback::Failed("el parámetro state no coincide (posible CSRF)".into());
    }
    match params.get("code") {
        Some(code) => Callback::Code(code.clone()),
        None => Callback::Failed("la respuesta de Twitch no incluye el code".into()),
    }
}

/// Atiende conexiones en `listener` hasta recibir el `code`. Responde una página mínima al
/// navegador y sigue esperando ante peticiones ajenas.
pub(crate) async fn wait_for_code(
    listener: &TcpListener,
    path: &str,
    state: &str,
) -> Result<String, OAuthError> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(OAuthError::Callback)?;
        let mut buffer = vec![0u8; 4096];
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(OAuthError::Callback)?;
        let (status, message, outcome) =
            match parse_callback(&String::from_utf8_lossy(&buffer[..read]), path, state) {
                Callback::Code(code) => (
                    "200 OK",
                    "Listo, ya puedes cerrar esta pestaña.",
                    Some(Ok(code)),
                ),
                Callback::Failed(reason) => (
                    "400 Bad Request",
                    "No se pudo autorizar. Revisa SYON.",
                    Some(Err(reason)),
                ),
                Callback::Unrelated => ("404 Not Found", "No encontrado.", None),
            };
        let body = format!("<!doctype html><meta charset=utf-8><p>{message}</p>");
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        // Si el navegador ya cerró la conexión no hay nada útil que hacer con el error.
        stream.write_all(reply.as_bytes()).await.ok();
        match outcome {
            Some(Ok(code)) => return Ok(code),
            Some(Err(reason)) => return Err(OAuthError::Authorization(reason)),
            None => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = "GET /auth/callback?code=abc&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";

    #[test]
    fn acepta_el_code_con_el_state_correcto() {
        assert_eq!(
            parse_callback(OK, "/auth/callback", "xyz"),
            Callback::Code("abc".into())
        );
    }

    #[test]
    fn rechaza_un_state_distinto() {
        assert!(matches!(
            parse_callback(OK, "/auth/callback", "otro"),
            Callback::Failed(m) if m.contains("CSRF")
        ));
    }

    #[test]
    fn ignora_otras_rutas_y_otros_metodos() {
        assert_eq!(
            parse_callback("GET /favicon.ico HTTP/1.1\r\n\r\n", "/auth/callback", "xyz"),
            Callback::Unrelated
        );
        assert_eq!(
            parse_callback(
                "POST /auth/callback HTTP/1.1\r\n\r\n",
                "/auth/callback",
                "xyz"
            ),
            Callback::Unrelated
        );
        assert_eq!(
            parse_callback("", "/auth/callback", "xyz"),
            Callback::Unrelated
        );
    }

    #[test]
    fn informa_del_rechazo_de_twitch_y_de_la_falta_de_code() {
        let denegado = "GET /auth/callback?error=access_denied&state=xyz HTTP/1.1\r\n\r\n";
        assert!(matches!(
            parse_callback(denegado, "/auth/callback", "xyz"),
            Callback::Failed(m) if m.contains("access_denied")
        ));
        let sin_code = "GET /auth/callback?state=xyz HTTP/1.1\r\n\r\n";
        assert!(matches!(
            parse_callback(sin_code, "/auth/callback", "xyz"),
            Callback::Failed(m) if m.contains("code")
        ));
    }
}
