use serde::Deserialize;
use syon_app::Credentials;
use syon_domain::Timestamp;

/// Margen antes del vencimiento a partir del cual el token se renueva.
const REFRESH_SKEW_MS: i64 = 60_000;

/// Permisos concedidos tal como los entrega Twitch: una lista JSON. Se acepta también una cadena
/// separada por espacios, que es la forma estándar de OAuth, por si Twitch la usara.
#[derive(Deserialize)]
#[serde(untagged)]
enum Scopes {
    List(Vec<String>),
    Text(String),
}

impl Scopes {
    fn into_list(self) -> Vec<String> {
        match self {
            Self::List(list) => list,
            Self::Text(text) => text.split_whitespace().map(str::to_owned).collect(),
        }
    }
}

/// Respuesta del endpoint de tokens de Twitch. No implementa `Debug`: contiene tokens.
#[derive(Deserialize)]
pub(crate) struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: Option<Scopes>,
}

impl TokenResponse {
    /// Convierte la respuesta en credenciales. Si Twitch no entrega un `refresh_token` nuevo se
    /// conserva el anterior.
    pub(crate) fn into_credentials(
        self,
        now: Timestamp,
        previous_refresh: Option<&str>,
    ) -> Credentials {
        let lifetime_ms = i64::try_from(self.expires_in.saturating_mul(1000)).unwrap_or(i64::MAX);
        Credentials {
            access_token: self.access_token,
            refresh_token: self
                .refresh_token
                .or_else(|| previous_refresh.map(str::to_owned)),
            expires_at: Some(Timestamp::from_unix_ms(
                now.as_unix_ms().saturating_add(lifetime_ms),
            )),
            scopes: self.scope.map(Scopes::into_list).unwrap_or_default(),
        }
    }
}

/// `true` si el token vence dentro del margen. Sin fecha de vencimiento se asume vigente; si Twitch
/// lo rechaza se renueva a la fuerza.
pub(crate) fn needs_refresh(credentials: &Credentials, now: Timestamp) -> bool {
    credentials.expires_at.is_some_and(|expires_at| {
        now.as_unix_ms().saturating_add(REFRESH_SKEW_MS) >= expires_at.as_unix_ms()
    })
}

/// `true` si los permisos concedidos incluyen todos los pedidos. Si Twitch no informó los
/// concedidos no se puede comprobar, así que se asumen válidos.
pub(crate) fn covers(credentials: &Credentials, requested: &[String]) -> bool {
    credentials.scopes.is_empty()
        || requested
            .iter()
            .all(|scope| credentials.scopes.contains(scope))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials(expires_at_ms: Option<i64>, scopes: &[&str]) -> Credentials {
        Credentials {
            access_token: "a".to_owned(),
            refresh_token: Some("r".to_owned()),
            expires_at: expires_at_ms.map(Timestamp::from_unix_ms),
            scopes: scopes.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn response(refresh: Option<&str>, scope: Option<Scopes>) -> TokenResponse {
        TokenResponse {
            access_token: "nuevo".to_owned(),
            refresh_token: refresh.map(str::to_owned),
            expires_in: 100,
            scope,
        }
    }

    #[test]
    fn la_respuesta_calcula_el_vencimiento_y_toma_los_permisos_de_la_lista() {
        let scope = Scopes::List(vec!["user:read:chat".into(), "user:write:chat".into()]);
        let credentials = response(Some("r2"), Some(scope))
            .into_credentials(Timestamp::from_unix_ms(1_000), None);
        assert_eq!(
            credentials.expires_at,
            Some(Timestamp::from_unix_ms(101_000))
        );
        assert_eq!(credentials.scopes, ["user:read:chat", "user:write:chat"]);
        assert_eq!(credentials.refresh_token.as_deref(), Some("r2"));
    }

    #[test]
    fn lee_los_permisos_como_lista_json_y_tambien_como_texto() {
        let lista: TokenResponse = serde_json::from_str(
            r#"{"access_token":"A","refresh_token":"R","expires_in":60,"scope":["a:b","c:d"],"token_type":"bearer"}"#,
        )
        .unwrap();
        let from_list = lista.into_credentials(Timestamp::from_unix_ms(0), None);
        assert_eq!(from_list.scopes, ["a:b", "c:d"]);

        let texto: TokenResponse =
            serde_json::from_str(r#"{"access_token":"A","expires_in":60,"scope":"a:b c:d"}"#)
                .unwrap();
        let from_text = texto.into_credentials(Timestamp::from_unix_ms(0), None);
        assert_eq!(from_text.scopes, ["a:b", "c:d"]);
    }

    #[test]
    fn conserva_el_refresh_token_anterior_si_no_llega_uno_nuevo() {
        let credentials =
            response(None, None).into_credentials(Timestamp::from_unix_ms(0), Some("viejo"));
        assert_eq!(credentials.refresh_token.as_deref(), Some("viejo"));
        assert!(credentials.scopes.is_empty());
    }

    #[test]
    fn un_vencimiento_enorme_se_satura_en_lugar_de_desbordar() {
        let mut huge = response(None, None);
        huge.expires_in = u64::MAX;
        let credentials = huge.into_credentials(Timestamp::from_unix_ms(5), None);
        assert_eq!(
            credentials.expires_at,
            Some(Timestamp::from_unix_ms(i64::MAX))
        );
    }

    #[test]
    fn se_renueva_dentro_del_margen_de_un_minuto() {
        let c = credentials(Some(100_000), &[]);
        assert!(!needs_refresh(&c, Timestamp::from_unix_ms(30_000)));
        assert!(needs_refresh(&c, Timestamp::from_unix_ms(40_000)));
        assert!(needs_refresh(&c, Timestamp::from_unix_ms(150_000)));
    }

    #[test]
    fn sin_fecha_de_vencimiento_se_asume_vigente() {
        assert!(!needs_refresh(
            &credentials(None, &[]),
            Timestamp::from_unix_ms(i64::MAX)
        ));
    }

    #[test]
    fn los_permisos_concedidos_deben_incluir_los_pedidos() {
        let c = credentials(None, &["user:read:chat", "user:write:chat"]);
        assert!(covers(&c, &["user:write:chat".to_owned()]));
        assert!(!covers(&c, &["channel:moderate".to_owned()]));
        assert!(covers(&credentials(None, &[]), &["cualquiera".to_owned()]));
    }
}
