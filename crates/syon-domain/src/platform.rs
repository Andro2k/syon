/// Plataforma de streaming de la que procede un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Kick.
    Kick,
    /// Twitch.
    Twitch,
}
