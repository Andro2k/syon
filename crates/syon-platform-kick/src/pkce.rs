use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

/// Par PKCE (RFC 7636, método S256): el `verifier` se guarda y el `challenge` viaja en la URL de
/// autorización.
pub(crate) struct Pkce {
    pub(crate) verifier: String,
    pub(crate) challenge: String,
}

impl Pkce {
    /// Genera un par nuevo con 32 bytes aleatorios.
    pub(crate) fn generate() -> Self {
        Self::from_verifier(random_token(32))
    }

    pub(crate) fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

/// Texto aleatorio apto para URL, con `bytes` bytes de entropía.
pub(crate) fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::fill(&mut buffer[..]);
    URL_SAFE_NO_PAD.encode(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cumple_el_vector_de_la_rfc_7636() {
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(
            pkce.challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn el_verifier_generado_es_valido_y_distinto_cada_vez() {
        let generados: HashSet<String> = (0..20).map(|_| Pkce::generate().verifier).collect();
        assert_eq!(generados.len(), 20);
        assert!(generados.iter().all(|v| (43..=128).contains(&v.len())));
    }
}
