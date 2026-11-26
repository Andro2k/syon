//! Adaptador de [`CredentialStore`](syon_app::CredentialStore) sobre un archivo `credentials.toml`
//! (ADR-0012). Los tokens son secretos: ningún tipo ni error de este crate los imprime.

#![warn(missing_docs)]

mod format;
mod store;

pub use store::TomlCredentialStore;
