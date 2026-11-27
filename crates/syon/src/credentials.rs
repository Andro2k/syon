//! Credenciales de la aplicación (ADR-0018). Es el único sitio que toca el entorno: el engine y
//! los adaptadores las reciben por constructor.

use std::env;
use syon_domain::Platform;
use syon_engine::AppCredentials;

/// Un valor vacío o en blanco cuenta como ausente.
fn present(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Nombres de las variables de entorno de una plataforma: `(client_id, client_secret)`.
pub fn variable_names(platform: Platform) -> (&'static str, &'static str) {
    match platform {
        Platform::Kick => ("SYON_KICK_CLIENT_ID", "SYON_KICK_CLIENT_SECRET"),
        Platform::Twitch => ("SYON_TWITCH_CLIENT_ID", "SYON_TWITCH_CLIENT_SECRET"),
    }
}

/// Valores incrustados al compilar con `option_env!`, `(client_id, client_secret)`.
fn embedded(platform: Platform) -> (Option<&'static str>, Option<&'static str>) {
    match platform {
        Platform::Kick => (
            option_env!("SYON_KICK_CLIENT_ID"),
            option_env!("SYON_KICK_CLIENT_SECRET"),
        ),
        Platform::Twitch => (
            option_env!("SYON_TWITCH_CLIENT_ID"),
            option_env!("SYON_TWITCH_CLIENT_SECRET"),
        ),
    }
}

/// Por qué no hay credenciales utilizables. Los mensajes nombran las variables, nunca su valor.
#[derive(Debug, PartialEq, Eq)]
pub enum Missing {
    /// No hay credenciales ni en el entorno ni incrustadas.
    Absent {
        /// Variable del `client_id`.
        id_variable: &'static str,
        /// Variable del `client_secret`.
        secret_variable: &'static str,
    },
    /// Solo una de las dos variables de entorno tiene valor.
    HalfSet {
        /// La variable que falta.
        missing: &'static str,
    },
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent {
                id_variable,
                secret_variable,
            } => write!(
                f,
                "esta compilación no incluye credenciales; defínelas en el entorno con \
                 {id_variable} y {secret_variable}"
            ),
            Self::HalfSet { missing } => write!(
                f,
                "solo una de las dos variables de la aplicación está definida; falta {missing}"
            ),
        }
    }
}

/// Elige las credenciales: el entorno en ejecución gana a lo incrustado. Se decide por pareja y no
/// por variable, para no mezclar el `client_id` de una aplicación con el secreto de otra. Una
/// pareja de entorno a medias es un error en lugar de caer en silencio a lo incrustado.
pub fn resolve(
    platform: Platform,
    runtime: (Option<&str>, Option<&str>),
    embedded: (Option<&str>, Option<&str>),
) -> Result<AppCredentials, Missing> {
    let (id_variable, secret_variable) = variable_names(platform);
    let pair = |(id, secret): (Option<&str>, Option<&str>)| {
        Some(AppCredentials {
            client_id: present(id)?.to_owned(),
            client_secret: present(secret)?.to_owned(),
        })
    };
    match (present(runtime.0), present(runtime.1)) {
        (Some(_), None) => {
            return Err(Missing::HalfSet {
                missing: secret_variable,
            });
        }
        (None, Some(_)) => {
            return Err(Missing::HalfSet {
                missing: id_variable,
            });
        }
        (Some(_), Some(_)) => return Ok(pair(runtime).expect("ambos presentes")),
        (None, None) => {}
    }
    pair(embedded).ok_or(Missing::Absent {
        id_variable,
        secret_variable,
    })
}

/// Credenciales de `platform` según el entorno actual y lo incrustado al compilar.
pub fn for_platform(platform: Platform) -> Result<AppCredentials, Missing> {
    let (id_variable, secret_variable) = variable_names(platform);
    let id = env::var(id_variable).ok();
    let secret = env::var(secret_variable).ok();
    resolve(
        platform,
        (id.as_deref(), secret.as_deref()),
        embedded(platform),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const KICK: Platform = Platform::Kick;

    #[test]
    fn el_entorno_gana_a_lo_incrustado() {
        let credentials = resolve(
            KICK,
            (Some("id-env"), Some("sec-env")),
            (Some("id-emb"), Some("sec-emb")),
        )
        .unwrap();
        assert_eq!(credentials.client_id, "id-env");
        assert_eq!(credentials.client_secret, "sec-env");
    }

    #[test]
    fn sin_entorno_se_usa_lo_incrustado() {
        let credentials = resolve(KICK, (None, None), (Some("id-emb"), Some("sec-emb"))).unwrap();
        assert_eq!(credentials.client_id, "id-emb");
    }

    #[test]
    fn un_valor_en_blanco_cuenta_como_ausente() {
        let credentials = resolve(
            KICK,
            (Some("  "), Some("")),
            (Some(" id-emb "), Some("sec-emb")),
        )
        .unwrap();
        assert_eq!(credentials.client_id, "id-emb");
        assert!(resolve(KICK, (None, None), (Some(" "), Some("x"))).is_err());
    }

    #[test]
    fn una_pareja_de_entorno_a_medias_es_un_error_y_no_cae_a_lo_incrustado() {
        let error = resolve(
            KICK,
            (Some("id-env"), None),
            (Some("id-emb"), Some("sec-emb")),
        )
        .unwrap_err();
        assert_eq!(
            error,
            Missing::HalfSet {
                missing: "SYON_KICK_CLIENT_SECRET"
            }
        );
        let error = resolve(KICK, (None, Some("sec-env")), (None, None)).unwrap_err();
        assert_eq!(
            error,
            Missing::HalfSet {
                missing: "SYON_KICK_CLIENT_ID"
            }
        );
    }

    #[test]
    fn la_ausencia_nombra_las_variables_y_no_sus_valores() {
        let error = resolve(Platform::Twitch, (None, None), (None, None)).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("SYON_TWITCH_CLIENT_ID"), "{text}");
        assert!(text.contains("SYON_TWITCH_CLIENT_SECRET"), "{text}");
        let half = resolve(KICK, (Some("valor-secreto"), None), (None, None)).unwrap_err();
        assert!(!half.to_string().contains("valor-secreto"));
    }
}
