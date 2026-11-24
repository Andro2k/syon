//! Entrada por consola: cada línea escrita se envía al chat como mensaje.

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{api::KickApi, console::Console};

fn message_from(line: &str) -> Option<&str> {
    let line = line.trim();
    (!line.is_empty()).then_some(line)
}

/// Lee la entrada estándar hasta que se cierre. Los mensajes enviados vuelven por el feed.
pub async fn forward_stdin(api: KickApi, broadcaster_user_id: u64, console: Console) {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Some(message) = message_from(&line) else {
            continue;
        };
        if let Err(error) = api.send_chat(broadcaster_user_id, message).await {
            console.status(&format!("No se pudo enviar el mensaje: {error:#}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignora_lineas_vacias_y_recorta_espacios() {
        assert_eq!(message_from("  hola mundo \n"), Some("hola mundo"));
        assert_eq!(message_from("   "), None);
        assert_eq!(message_from(""), None);
    }
}
