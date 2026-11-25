/// Persona que escribe o actúa en una plataforma, identificada por su nombre visible.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct User {
    /// Nombre visible tal como lo entrega la plataforma.
    pub name: String,
}

impl User {
    /// Crea un usuario a partir de su nombre visible.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
