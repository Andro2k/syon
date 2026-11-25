//! Adaptador de [`SettingsStore`](syon_app::SettingsStore) sobre un archivo `settings.toml`.

#![warn(missing_docs)]

mod format;
mod store;

pub use store::TomlSettingsStore;
