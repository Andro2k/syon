use serde::{Deserialize, Serialize, de::DeserializeOwned};
use syon_app::{ChatCommand, ChatSettings, ChatUserFilter, LogLevel, MinimumChatRole, Settings};
use syon_domain::Platform;
use toml::{Table, Value};

/// Forma de escritura de `settings.toml`. Es propia del adaptador: `syon-app` no conoce
/// `serde`, así que el modelo se traduce hacia este tipo al guardar y se lee clave a
/// clave al cargar (ver [`parse`]).
#[derive(Serialize)]
struct SettingsFile {
    schema_version: u32,
    logging: LoggingSection,
    kick: KickSection,
    twitch: TwitchSection,
    chat: ChatSection,
}

/// Sección `[logging]`.
#[derive(Serialize)]
struct LoggingSection {
    level: LevelName,
}

/// Sección `[twitch]`: solo indica si la plataforma está activada.
#[derive(Serialize)]
struct TwitchSection {
    enabled: bool,
}

/// Sección `[kick]`. Los ids opcionales se omiten cuando no están definidos.
#[derive(Serialize)]
struct KickSection {
    enabled: bool,
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chatroom_id: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct ChatSection {
    prefix: String,
    commands: Vec<CommandSection>,
    ignored_users: Vec<UserFilterSection>,
    required_phrases: Vec<String>,
    blocked_phrases: Vec<String>,
    duplicate_window_seconds: u64,
    command_cooldown_seconds: u64,
    response_cooldown_seconds: u64,
    platform_interval_seconds: u64,
}

impl Default for ChatSection {
    fn default() -> Self {
        let settings = ChatSettings::default();
        Self {
            prefix: settings.prefix,
            commands: Vec::new(),
            ignored_users: Vec::new(),
            required_phrases: Vec::new(),
            blocked_phrases: Vec::new(),
            duplicate_window_seconds: settings.duplicate_window_seconds,
            command_cooldown_seconds: settings.command_cooldown_seconds,
            response_cooldown_seconds: settings.response_cooldown_seconds,
            platform_interval_seconds: settings.platform_interval_seconds,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CommandSection {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    response: String,
    #[serde(default = "enabled_default")]
    enabled: bool,
    #[serde(default)]
    platforms: Vec<String>,
    #[serde(default)]
    destinations: Vec<String>,
    #[serde(default = "everyone_name")]
    minimum_role: String,
}

fn enabled_default() -> bool {
    true
}

fn everyone_name() -> String {
    "everyone".into()
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct UserFilterSection {
    author_id: Option<String>,
    name: Option<String>,
}
fn role_name(role: MinimumChatRole) -> String {
    match role {
        MinimumChatRole::Everyone => "everyone",
        MinimumChatRole::Subscriber => "subscriber",
        MinimumChatRole::Vip => "vip",
        MinimumChatRole::Moderator => "moderator",
        MinimumChatRole::Broadcaster => "broadcaster",
    }
    .into()
}

impl From<&ChatSettings> for ChatSection {
    fn from(settings: &ChatSettings) -> Self {
        Self {
            prefix: settings.prefix.clone(),
            commands: settings
                .commands
                .iter()
                .map(|command| CommandSection {
                    name: command.name.clone(),
                    aliases: command.aliases.clone(),
                    response: command.response.clone(),
                    enabled: command.enabled,
                    platforms: command
                        .platforms
                        .iter()
                        .map(|p| {
                            match p {
                                Platform::Kick => "kick",
                                Platform::Twitch => "twitch",
                            }
                            .to_string()
                        })
                        .collect(),
                    destinations: command.destinations.iter().map(platform_name).collect(),
                    minimum_role: role_name(command.minimum_role),
                })
                .collect(),
            ignored_users: settings
                .ignored_users
                .iter()
                .map(|user| UserFilterSection {
                    author_id: user.author_id.clone(),
                    name: user.name.clone(),
                })
                .collect(),
            required_phrases: settings.required_phrases.clone(),
            blocked_phrases: settings.blocked_phrases.clone(),
            duplicate_window_seconds: settings.duplicate_window_seconds,
            command_cooldown_seconds: settings.command_cooldown_seconds,
            response_cooldown_seconds: settings.response_cooldown_seconds,
            platform_interval_seconds: settings.platform_interval_seconds,
        }
    }
}

impl TryFrom<ChatSection> for ChatSettings {
    type Error = String;
    fn try_from(section: ChatSection) -> Result<Self, Self::Error> {
        if section.prefix.trim().is_empty() || section.prefix.chars().count() > 4 {
            return Err("chat.prefix: debe tener entre 1 y 4 caracteres".into());
        }
        if section.duplicate_window_seconds == 0 || section.duplicate_window_seconds > 3600 {
            return Err("chat.duplicate_window_seconds: debe estar entre 1 y 3600".into());
        }
        if section.command_cooldown_seconds == 0 || section.command_cooldown_seconds > 3600 {
            return Err("chat.command_cooldown_seconds: debe estar entre 1 y 3600".into());
        }
        if section.response_cooldown_seconds == 0 || section.response_cooldown_seconds > 3600 {
            return Err("chat.response_cooldown_seconds: debe estar entre 1 y 3600".into());
        }
        if section.platform_interval_seconds == 0 || section.platform_interval_seconds > 60 {
            return Err("chat.platform_interval_seconds: debe estar entre 1 y 60".into());
        }
        for (key, phrases) in [
            ("chat.required_phrases", &section.required_phrases),
            ("chat.blocked_phrases", &section.blocked_phrases),
        ] {
            if phrases
                .iter()
                .any(|phrase| phrase.trim().is_empty() || phrase.chars().count() > 200)
            {
                return Err(format!(
                    "{key}: las frases deben tener entre 1 y 200 caracteres"
                ));
            }
        }
        let mut seen = std::collections::HashSet::new();
        let mut commands = Vec::new();
        for command in section.commands {
            if command.name.trim().is_empty() || command.name.chars().count() > 64 {
                return Err(
                    "chat.commands.name: no puede estar vacío ni superar 64 caracteres".into(),
                );
            }
            if command.response.trim().is_empty() || command.response.chars().count() > 500 {
                return Err(format!(
                    "chat.commands.{}: respuesta vacía o demasiado larga",
                    command.name
                ));
            }
            let mut names = vec![command.name.clone()];
            names.extend(command.aliases.iter().cloned());
            for name in &names {
                if name.trim().is_empty() || name.chars().count() > 64 {
                    return Err("chat.commands.aliases: nombre vacío o demasiado largo".into());
                }
                if !seen.insert(name.to_lowercase()) {
                    return Err(format!("chat.commands: nombre o alias duplicado: {name}"));
                }
            }
            let mut platforms = Vec::new();
            for platform in &command.platforms {
                let parsed = match platform.as_str() {
                    "kick" => Platform::Kick,
                    "twitch" => Platform::Twitch,
                    _ => {
                        return Err(format!(
                            "chat.commands.{}.platforms: plataforma desconocida {platform}",
                            command.name
                        ));
                    }
                };
                if !platforms.contains(&parsed) {
                    platforms.push(parsed);
                }
            }
            let destinations =
                parse_platforms(&command.destinations, &command.name, "destinations")?;
            let minimum_role = match command.minimum_role.as_str() {
                "everyone" => MinimumChatRole::Everyone,
                "subscriber" => MinimumChatRole::Subscriber,
                "vip" => MinimumChatRole::Vip,
                "moderator" => MinimumChatRole::Moderator,
                "broadcaster" => MinimumChatRole::Broadcaster,
                other => {
                    return Err(format!(
                        "chat.commands.{}.minimum_role: rol desconocido {other}",
                        command.name
                    ));
                }
            };
            commands.push(ChatCommand {
                name: command.name,
                aliases: command.aliases,
                response: command.response,
                enabled: command.enabled,
                platforms,
                destinations,
                minimum_role,
            });
        }
        for user in &section.ignored_users {
            if user.author_id.as_deref().unwrap_or_default().is_empty()
                && user.name.as_deref().unwrap_or_default().trim().is_empty()
            {
                return Err("chat.ignored_users: cada filtro necesita author_id o name".into());
            }
        }
        Ok(Self {
            prefix: section.prefix,
            commands,
            ignored_users: section
                .ignored_users
                .into_iter()
                .map(|u| ChatUserFilter {
                    author_id: u.author_id,
                    name: u.name,
                })
                .collect(),
            required_phrases: section.required_phrases,
            blocked_phrases: section.blocked_phrases,
            duplicate_window_seconds: section.duplicate_window_seconds,
            command_cooldown_seconds: section.command_cooldown_seconds,
            response_cooldown_seconds: section.response_cooldown_seconds,
            platform_interval_seconds: section.platform_interval_seconds,
        })
    }
}

fn platform_name(platform: &Platform) -> String {
    match platform {
        Platform::Kick => "kick",
        Platform::Twitch => "twitch",
    }
    .into()
}

fn parse_platforms(values: &[String], name: &str, field: &str) -> Result<Vec<Platform>, String> {
    values
        .iter()
        .map(|platform| match platform.as_str() {
            "kick" => Ok(Platform::Kick),
            "twitch" => Ok(Platform::Twitch),
            _ => Err(format!(
                "chat.commands.{name}.{field}: plataforma desconocida {platform}"
            )),
        })
        .collect()
}

/// Nombre del nivel de log tal como se escribe en el archivo.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LevelName {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for LevelName {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Self::Error,
            LogLevel::Warn => Self::Warn,
            LogLevel::Info => Self::Info,
            LogLevel::Debug => Self::Debug,
            LogLevel::Trace => Self::Trace,
        }
    }
}

impl From<LevelName> for LogLevel {
    fn from(name: LevelName) -> Self {
        match name {
            LevelName::Error => Self::Error,
            LevelName::Warn => Self::Warn,
            LevelName::Info => Self::Info,
            LevelName::Debug => Self::Debug,
            LevelName::Trace => Self::Trace,
        }
    }
}

/// Convierte los ajustes al texto TOML que se escribe en disco.
pub(crate) fn render(settings: &Settings) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(&SettingsFile {
        schema_version: settings.schema_version,
        logging: LoggingSection {
            level: settings.logging.level.into(),
        },
        kick: KickSection {
            enabled: settings.kick.enabled,
            channel: settings.kick.channel.clone(),
            channel_id: settings.kick.channel_id,
            chatroom_id: settings.kick.chatroom_id,
        },
        twitch: TwitchSection {
            enabled: settings.twitch.enabled,
        },
        chat: ChatSection::from(&settings.chat),
    })
}

/// Lee los ajustes de un texto TOML. Las claves ausentes toman su valor por defecto y las
/// desconocidas se ignoran. Se lee clave a clave, en lugar de derivar `Deserialize` sobre
/// todo el archivo, porque así un tipo incorrecto nombra la clave (`logging.level: …`),
/// algo que los errores de `toml` no hacen por sí solos.
pub(crate) fn parse(text: &str) -> Result<Settings, String> {
    let table: Table = text
        .parse()
        .map_err(|error: toml::de::Error| error.to_string())?;
    let mut settings = Settings::default();
    if let Some(version) = read::<u32>(&table, &["schema_version"])? {
        settings.schema_version = version;
    }
    if let Some(level) = read::<LevelName>(&table, &["logging", "level"])? {
        settings.logging.level = level.into();
    }
    if let Some(enabled) = read::<bool>(&table, &["kick", "enabled"])? {
        settings.kick.enabled = enabled;
    }
    if let Some(channel) = read::<String>(&table, &["kick", "channel"])? {
        settings.kick.channel = channel;
    }
    settings.kick.channel_id = read::<u64>(&table, &["kick", "channel_id"])?;
    settings.kick.chatroom_id = read::<u64>(&table, &["kick", "chatroom_id"])?;
    if let Some(enabled) = read::<bool>(&table, &["twitch", "enabled"])? {
        settings.twitch.enabled = enabled;
    }
    let chat: ChatSection = read::<ChatSection>(&table, &["chat"])?.unwrap_or_default();
    settings.chat = chat.try_into()?;
    Ok(settings)
}

/// Lee solo el `schema_version` de un texto TOML, sin validar el resto. Devuelve `None` si el
/// texto no es TOML o la clave falta o no es un entero, para que quien guarda no se bloquee por
/// un archivo que `load` ya rechaza por su cuenta.
pub(crate) fn schema_version(text: &str) -> Option<u32> {
    let table: Table = text.parse().ok()?;
    read::<u32>(&table, &["schema_version"]).ok().flatten()
}

/// Busca `path` (por ejemplo `["logging", "level"]`) y lo convierte a `T`. Devuelve `None`
/// si la clave no existe; si existe con otro tipo, el error nombra la clave completa.
fn read<T: DeserializeOwned>(table: &Table, path: &[&str]) -> Result<Option<T>, String> {
    let mut current = table;
    let (last, parents) = path
        .split_last()
        .expect("la ruta de una clave no está vacía");
    for (depth, segment) in parents.iter().enumerate() {
        match current.get(*segment) {
            None => return Ok(None),
            Some(Value::Table(inner)) => current = inner,
            Some(other) => {
                let key = path[..=depth].join(".");
                return Err(format!(
                    "{key}: se esperaba una tabla y es {}",
                    other.type_str()
                ));
            }
        }
    }
    match current.get(*last) {
        None => Ok(None),
        Some(value) => value
            .clone()
            .try_into()
            .map(Some)
            .map_err(|error: toml::de::Error| format!("{}: {}", path.join("."), error.message())),
    }
}
