//! Cliente de la API pública oficial de Kick (`api.kick.com/public/v1`) con token de usuario.

use anyhow::{Result, anyhow, bail};
use reqwest::{Client, Method, Response, StatusCode};
use serde_json::{Value, json};

use crate::oauth::TokenManager;

const API_BASE: &str = "https://api.kick.com/public/v1";

#[derive(Debug, Clone, PartialEq)]
pub struct Me {
    pub user_id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelInfo {
    pub broadcaster_user_id: u64,
    pub slug: String,
    pub title: String,
    pub is_live: bool,
}

pub struct KickApi {
    http: Client,
    tokens: TokenManager,
}

impl KickApi {
    pub fn new(http: Client, tokens: TokenManager) -> Self {
        Self { http, tokens }
    }

    /// Usuario dueño del token (scope `user:read`).
    pub async fn me(&self) -> Result<Me> {
        parse_me(&self.request(Method::GET, "/users", &[], None).await?)
    }

    /// Datos públicos de un canal (scope `channel:read`).
    pub async fn channel(&self, slug: &str) -> Result<ChannelInfo> {
        parse_channel(
            &self
                .request(Method::GET, "/channels", &[("slug", slug)], None)
                .await?,
        )
    }

    /// Envía un mensaje al chat de un canal como el usuario autenticado (scope `chat:write`).
    pub async fn send_chat(&self, broadcaster_user_id: u64, content: &str) -> Result<()> {
        let body = json!({ "broadcaster_user_id": broadcaster_user_id, "content": content, "type": "user" });
        check_sent(
            &self
                .request(Method::POST, "/chat", &[], Some(&body))
                .await?,
        )
    }

    /// Envía la petición y, si Kick responde 401, renueva el token y reintenta una vez.
    async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<Value> {
        let token = self.tokens.access_token().await?;
        let mut response = self.send_once(&method, path, query, body, &token).await?;
        if response.status() == StatusCode::UNAUTHORIZED {
            let renewed = self.tokens.force_refresh().await?;
            response = self.send_once(&method, path, query, body, &renewed).await?;
        }
        read_json(response).await
    }

    async fn send_once(
        &self,
        method: &Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
        token: &str,
    ) -> Result<Response> {
        let mut request = self
            .http
            .request(method.clone(), format!("{API_BASE}{path}"))
            .bearer_auth(token)
            .query(query);
        if let Some(body) = body {
            request = request.json(body);
        }
        Ok(request.send().await?)
    }
}

async fn read_json(response: Response) -> Result<Value> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        let detail: String = text.chars().take(200).collect();
        bail!("Kick respondió {status}: {detail}");
    }
    Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn first_item(body: &Value) -> Result<&Value> {
    body["data"]
        .get(0)
        .ok_or_else(|| anyhow!("Kick no devolvió datos en la respuesta"))
}

fn parse_me(body: &Value) -> Result<Me> {
    let user = first_item(body)?;
    Ok(Me {
        user_id: user["user_id"]
            .as_u64()
            .ok_or_else(|| anyhow!("La respuesta no incluye user_id"))?,
        name: user["name"].as_str().unwrap_or("desconocido").to_string(),
    })
}

fn parse_channel(body: &Value) -> Result<ChannelInfo> {
    let channel = first_item(body)?;
    Ok(ChannelInfo {
        broadcaster_user_id: channel["broadcaster_user_id"]
            .as_u64()
            .ok_or_else(|| anyhow!("La respuesta no incluye broadcaster_user_id"))?,
        slug: channel["slug"].as_str().unwrap_or_default().to_string(),
        title: channel["stream_title"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        is_live: channel["stream"]["is_live"].as_bool().unwrap_or(false),
    })
}

fn check_sent(body: &Value) -> Result<()> {
    if body["data"]["is_sent"] == json!(false) {
        bail!("Kick no envió el mensaje");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_usuario_y_canal() {
        let me = json!({ "data": [{ "user_id": 42, "name": "luis", "email": "no-se-usa" }], "message": "OK" });
        assert_eq!(
            parse_me(&me).unwrap(),
            Me {
                user_id: 42,
                name: "luis".into()
            }
        );

        let canal = json!({ "data": [{
            "broadcaster_user_id": 7, "slug": "luis", "stream_title": "Hola",
            "stream": { "is_live": true, "viewer_count": 3 }
        }] });
        assert_eq!(
            parse_channel(&canal).unwrap(),
            ChannelInfo {
                broadcaster_user_id: 7,
                slug: "luis".into(),
                title: "Hola".into(),
                is_live: true
            }
        );
    }

    #[test]
    fn el_canal_sin_directo_figura_como_offline() {
        let canal = json!({ "data": [{ "broadcaster_user_id": 7, "slug": "x" }] });
        assert!(!parse_channel(&canal).unwrap().is_live);
    }

    #[test]
    fn rechaza_respuestas_vacias_o_incompletas() {
        assert!(parse_me(&json!({ "data": [] })).is_err());
        assert!(parse_channel(&json!({})).is_err());
        assert!(parse_channel(&json!({ "data": [{ "slug": "x" }] })).is_err());
    }

    #[test]
    fn detecta_mensajes_no_enviados() {
        assert!(check_sent(&json!({ "data": { "is_sent": true, "message_id": "a" } })).is_ok());
        assert!(check_sent(&json!({ "data": { "is_sent": false } })).is_err());
    }
}
