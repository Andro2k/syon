//! Utilidades de prueba: un servidor HTTP local que responde con respuestas fijas y anota las
//! peticiones, para probar el OAuth sin red.

use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// Servidor HTTP local de prueba.
pub(crate) struct FakeServer {
    addr: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FakeServer {
    /// Dirección base del servidor, usable como endpoint de tokens.
    pub(crate) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Peticiones recibidas hasta ahora, completas (cabeceras y cuerpo).
    pub(crate) fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

/// Levanta un servidor que atiende una conexión por cada `(código, cuerpo)` de `responses`, en orden.
pub(crate) async fn fake_server(responses: Vec<(u16, &'static str)>) -> FakeServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    tokio::spawn(async move {
        for (status, body) in responses {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let request = read_request(&mut stream).await;
            seen.lock().unwrap().push(request);
            let reply = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    FakeServer { addr, requests }
}

/// Lee una petición HTTP completa: cabeceras y, si hay `Content-Length`, el cuerpo.
async fn read_request(stream: &mut TcpStream) -> String {
    let mut data = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.unwrap_or(0);
        if read == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            if data.len() >= end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}

/// Un puerto local libre (se suelta antes de devolverlo; basta para pruebas secuenciales).
pub(crate) fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
