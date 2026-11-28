use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use syon_app::{ChatCommand, ChatSettings, MinimumChatRole};
use syon_domain::{ChatMessage, ChatRoles, Event, EventKind, MessageId, Platform};

const MAX_TRACKED: usize = 4096;

/// Salida aceptada por las reglas de chat, aún sin realizar I/O de red.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChatResponse {
    pub platforms: Vec<Platform>,
    pub text: String,
}

/// Estado monotónico y acotado para duplicados y enfriamientos.
#[derive(Default)]
pub(crate) struct ChatAutomation {
    ids: VecDeque<(Platform, String, Instant)>,
    fingerprints: VecDeque<(Platform, String, Instant)>,
    cooldowns: VecDeque<(Platform, String, String, Instant)>,
    response_cooldowns: VecDeque<(Platform, String, Instant)>,
    last_send: [Option<Instant>; 2],
    echoes: VecDeque<(Platform, String, Option<String>, Instant)>,
}

impl ChatAutomation {
    /// Decide una respuesta para `event`; los mensajes propios o inciertos nunca avanzan el estado.
    pub(crate) fn process(
        &mut self,
        settings: &ChatSettings,
        event: &Event,
        now: Instant,
    ) -> Option<ChatResponse> {
        let EventKind::Chat(message) = &event.kind else {
            return None;
        };
        if message.is_self == Some(true) || self.consume_echo(event.platform, message, now) {
            return None;
        }
        if message.is_self != Some(false) || message.author_id.is_none() {
            return None;
        }
        self.prune(now, settings);
        if self.duplicate(event.platform, message, now) || filtered(settings, message) {
            return None;
        }
        let (name, args) = parse_command(&settings.prefix, &message.text)?;
        let command = settings.commands.iter().find(|command| {
            command.enabled && names(command).any(|candidate| candidate.eq_ignore_ascii_case(name))
        })?;
        if !command.platforms.is_empty() && !command.platforms.contains(&event.platform) {
            return None;
        }
        if !authorized(command.minimum_role, message.roles) {
            return None;
        }
        let user_key = message
            .author_id
            .as_deref()
            .unwrap_or_default()
            .to_lowercase();
        let command_key = command.name.to_lowercase();
        if self.cooldowns.iter().any(|(platform, user, name, at)| {
            *platform == event.platform
                && user == &user_key
                && name == &command_key
                && now.duration_since(*at) < Duration::from_secs(settings.command_cooldown_seconds)
        }) {
            return None;
        }
        let index = platform_index(event.platform);
        if self.last_send[index].is_some_and(|at| {
            now.duration_since(at) < Duration::from_secs(settings.platform_interval_seconds)
        }) {
            return None;
        }
        let mut text = command
            .response
            .replace("{user}", &message.author.name)
            .replace(
                "{platform}",
                match event.platform {
                    Platform::Kick => "kick",
                    Platform::Twitch => "twitch",
                },
            )
            .replace("{args}", args);
        text = text.trim().to_owned();
        if text.is_empty() {
            return None;
        }
        let response_key = text.to_lowercase();
        if self.response_cooldowns.iter().any(|(platform, value, at)| {
            *platform == event.platform
                && value == &response_key
                && now.duration_since(*at) < Duration::from_secs(settings.response_cooldown_seconds)
        }) {
            return None;
        }
        self.cooldowns
            .push_back((event.platform, user_key, command_key, now));
        self.response_cooldowns
            .push_back((event.platform, response_key, now));
        while self.response_cooldowns.len() > MAX_TRACKED {
            self.response_cooldowns.pop_front();
        }
        self.last_send[index] = Some(now);
        let platforms = if command.destinations.is_empty() {
            vec![event.platform]
        } else {
            command.destinations.clone()
        };
        Some(ChatResponse { platforms, text })
    }

    /// Registra el texto exacto antes de iniciar el envío para absorber ecos tempranos.
    pub(crate) fn register_send(&mut self, platform: Platform, text: &str, now: Instant) {
        self.echoes.push_back((
            platform,
            text.to_owned(),
            None,
            now + Duration::from_secs(30),
        ));
        while self.echoes.len() > MAX_TRACKED {
            self.echoes.pop_front();
        }
    }

    /// Asocia el id devuelto por la plataforma o elimina la correlación si el envío falló.
    pub(crate) fn complete_send(
        &mut self,
        platform: Platform,
        text: &str,
        id: Option<&MessageId>,
        succeeded: bool,
    ) {
        let Some(index) = self
            .echoes
            .iter()
            .rposition(|(p, value, id, _)| *p == platform && value == text && id.is_none())
        else {
            return;
        };
        if succeeded {
            self.echoes[index].2 = id.map(|value| value.as_str().to_owned());
        } else {
            self.echoes.remove(index);
        }
    }

    fn consume_echo(&mut self, platform: Platform, message: &ChatMessage, now: Instant) -> bool {
        self.echoes.retain(|(_, _, _, expires)| *expires > now);
        let index = self.echoes.iter().position(|(p, text, id, _)| {
            *p == platform && (id.as_deref() == Some(message.id.as_str()) || text == &message.text)
        });
        if let Some(index) = index {
            self.echoes.remove(index);
            true
        } else {
            false
        }
    }

    fn duplicate(&mut self, platform: Platform, message: &ChatMessage, now: Instant) -> bool {
        let fingerprint = format!(
            "{}:{}",
            message
                .author_id
                .as_deref()
                .unwrap_or_default()
                .to_lowercase(),
            message.text.to_lowercase()
        );
        let duplicate_id = self
            .ids
            .iter()
            .any(|(seen_platform, id, _)| *seen_platform == platform && id == message.id.as_str());
        let duplicate_content = self
            .fingerprints
            .iter()
            .any(|(seen_platform, value, _)| *seen_platform == platform && value == &fingerprint);
        self.ids
            .push_back((platform, message.id.as_str().to_owned(), now));
        self.fingerprints.push_back((platform, fingerprint, now));
        while self.ids.len() > MAX_TRACKED {
            self.ids.pop_front();
        }
        while self.fingerprints.len() > MAX_TRACKED {
            self.fingerprints.pop_front();
        }
        duplicate_id || duplicate_content
    }

    fn prune(&mut self, now: Instant, settings: &ChatSettings) {
        let duplicate_ttl = Duration::from_secs(settings.duplicate_window_seconds);
        while self
            .ids
            .front()
            .is_some_and(|(_, _, at)| now.duration_since(*at) >= duplicate_ttl)
        {
            self.ids.pop_front();
        }
        while self
            .fingerprints
            .front()
            .is_some_and(|(_, _, at)| now.duration_since(*at) >= duplicate_ttl)
        {
            self.fingerprints.pop_front();
        }
        let cooldown_ttl = Duration::from_secs(settings.command_cooldown_seconds);
        while self
            .cooldowns
            .front()
            .is_some_and(|(_, _, _, at)| now.duration_since(*at) >= cooldown_ttl)
        {
            self.cooldowns.pop_front();
        }
        while self.cooldowns.len() > MAX_TRACKED {
            self.cooldowns.pop_front();
        }
        while self.response_cooldowns.front().is_some_and(|(_, _, at)| {
            now.duration_since(*at) >= Duration::from_secs(settings.response_cooldown_seconds)
        }) {
            self.response_cooldowns.pop_front();
        }
    }
}

fn names(command: &ChatCommand) -> impl Iterator<Item = &str> {
    std::iter::once(command.name.as_str()).chain(command.aliases.iter().map(String::as_str))
}

fn parse_command<'a>(prefix: &str, text: &'a str) -> Option<(&'a str, &'a str)> {
    let body = text.strip_prefix(prefix)?;
    let body = body.trim_start_matches(char::is_whitespace);
    let split = body.find(char::is_whitespace).unwrap_or(body.len());
    let name = &body[..split];
    if name.is_empty() {
        return None;
    }
    Some((name, body[split..].trim()))
}

fn authorized(minimum: MinimumChatRole, roles: ChatRoles) -> bool {
    match minimum {
        MinimumChatRole::Everyone => true,
        MinimumChatRole::Subscriber => {
            roles.subscriber || roles.vip || roles.moderator || roles.broadcaster
        }
        MinimumChatRole::Vip => roles.vip || roles.moderator || roles.broadcaster,
        MinimumChatRole::Moderator => roles.moderator || roles.broadcaster,
        MinimumChatRole::Broadcaster => roles.broadcaster,
    }
}

fn filtered(settings: &ChatSettings, message: &ChatMessage) -> bool {
    if settings.ignored_users.iter().any(|user| {
        user.author_id.as_deref().is_some_and(|id| {
            message
                .author_id
                .as_deref()
                .is_some_and(|actual| actual.eq_ignore_ascii_case(id))
        }) || user
            .name
            .as_deref()
            .is_some_and(|name| message.author.name.eq_ignore_ascii_case(name))
    }) {
        return true;
    }
    let text = message.text.to_lowercase();
    if settings
        .blocked_phrases
        .iter()
        .any(|phrase| text.contains(&phrase.to_lowercase()))
    {
        return true;
    }
    if !settings.required_phrases.is_empty()
        && !settings
            .required_phrases
            .iter()
            .any(|phrase| text.contains(&phrase.to_lowercase()))
    {
        return true;
    }
    false
}

fn platform_index(platform: Platform) -> usize {
    match platform {
        Platform::Kick => 0,
        Platform::Twitch => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syon_domain::{MessageId, Timestamp, User};

    fn event(text: &str, id: &str, roles: ChatRoles) -> Event {
        Event {
            platform: Platform::Twitch,
            received_at: Timestamp::from_unix_ms(1),
            kind: EventKind::Chat(ChatMessage {
                id: MessageId::new(id),
                author: User::new("ana"),
                author_id: Some("42".into()),
                roles,
                is_self: Some(false),
                text: text.into(),
            }),
        }
    }

    fn command(role: MinimumChatRole) -> ChatCommand {
        ChatCommand {
            name: "saluda".into(),
            aliases: vec!["hi".into()],
            response: "Hola {user} {platform} {args}".into(),
            enabled: true,
            platforms: vec![],
            destinations: vec![],
            minimum_role: role,
        }
    }

    #[test]
    fn parser_acepta_argumentos_con_espacios_unicode_y_alias_sin_importar_mayusculas() {
        assert_eq!(
            parse_command("!", "!HOLA\tmañana　🙂"),
            Some(("HOLA", "mañana　🙂"))
        );
        assert_eq!(parse_command("!", "hola"), None);
    }

    #[test]
    fn jerarquia_acumulativa_de_roles_es_exacta() {
        let roles = [
            ChatRoles::default(),
            ChatRoles {
                subscriber: true,
                ..Default::default()
            },
            ChatRoles {
                vip: true,
                ..Default::default()
            },
            ChatRoles {
                moderator: true,
                ..Default::default()
            },
            ChatRoles {
                broadcaster: true,
                ..Default::default()
            },
        ];
        let levels = [
            MinimumChatRole::Everyone,
            MinimumChatRole::Subscriber,
            MinimumChatRole::Vip,
            MinimumChatRole::Moderator,
            MinimumChatRole::Broadcaster,
        ];
        let expected = [
            [true, false, false, false, false],
            [true, true, false, false, false],
            [true, true, true, false, false],
            [true, true, true, true, false],
            [true, true, true, true, true],
        ];
        for (r, roles) in roles.into_iter().enumerate() {
            for (l, level) in levels.into_iter().enumerate() {
                assert_eq!(authorized(level, roles), expected[r][l]);
            }
        }
    }

    #[test]
    fn propios_y_autoria_desconocida_no_se_despachan() {
        let mut settings = ChatSettings::default();
        settings.commands.push(command(MinimumChatRole::Everyone));
        let mut own = event("!saluda", "1", ChatRoles::default());
        if let EventKind::Chat(message) = &mut own.kind {
            message.is_self = Some(true);
        }
        let mut unknown = event("!saluda", "2", ChatRoles::default());
        if let EventKind::Chat(message) = &mut unknown.kind {
            message.is_self = None;
        }
        let mut automation = ChatAutomation::default();
        let now = Instant::now();
        assert!(automation.process(&settings, &own, now).is_none());
        assert!(automation.process(&settings, &unknown, now).is_none());
    }

    #[test]
    fn sustituciones_alias_filtros_deduplicacion_y_destino_de_origen() {
        let now = Instant::now();
        let mut settings = ChatSettings::default();
        settings.commands.push(command(MinimumChatRole::Everyone));
        let mut automation = ChatAutomation::default();
        let input = event("!HI mañana", "1", ChatRoles::default());
        assert_eq!(
            automation.process(&settings, &input, now),
            Some(ChatResponse {
                platforms: vec![Platform::Twitch],
                text: "Hola ana twitch mañana".into()
            })
        );
        assert!(
            automation
                .process(&settings, &input, now + Duration::from_secs(2))
                .is_none()
        );
        settings.blocked_phrases.push("!hi".into());
        assert!(
            ChatAutomation::default()
                .process(&settings, &input, now)
                .is_none()
        );
    }

    #[test]
    fn eco_propio_esperado_se_consume_una_vez_y_no_dispara_otra_respuesta() {
        let now = Instant::now();
        let mut settings = ChatSettings::default();
        settings.commands.push(command(MinimumChatRole::Everyone));
        let mut automation = ChatAutomation::default();
        automation.register_send(Platform::Twitch, "respuesta", now);
        let mut echo = event("respuesta", "echo-id", ChatRoles::default());
        if let EventKind::Chat(message) = &mut echo.kind {
            message.is_self = None;
            message.author_id = None;
        }
        assert!(automation.process(&settings, &echo, now).is_none());
        assert!(automation.process(&settings, &echo, now).is_none());
    }

    #[test]
    fn error_de_envio_elimina_la_correlacion_pendiente() {
        let now = Instant::now();
        let mut settings = ChatSettings::default();
        settings.commands.push(command(MinimumChatRole::Everyone));
        let mut automation = ChatAutomation::default();
        automation.register_send(Platform::Twitch, "respuesta", now);
        automation.complete_send(Platform::Twitch, "respuesta", None, false);
        let mut message = event("respuesta", "foreign", ChatRoles::default());
        if let EventKind::Chat(chat) = &mut message.kind {
            chat.is_self = Some(false);
        }
        assert!(automation.process(&settings, &message, now).is_none());
        let mut command_event = event("!saluda", "command", ChatRoles::default());
        if let EventKind::Chat(chat) = &mut command_event.kind {
            chat.is_self = Some(false);
        }
        assert!(automation.process(&settings, &command_event, now).is_some());
    }

    #[test]
    fn cooldown_por_usuario_y_comando_y_lista_de_origen_se_aplican() {
        let now = Instant::now();
        let mut settings = ChatSettings {
            duplicate_window_seconds: 1,
            ..ChatSettings::default()
        };
        let mut configured = command(MinimumChatRole::Everyone);
        configured.platforms = vec![Platform::Kick];
        settings.commands.push(configured);
        let mut automation = ChatAutomation::default();
        let mut first = event("!saluda uno", "1", ChatRoles::default());
        if let EventKind::Chat(message) = &mut first.kind {
            message.is_self = Some(false);
        }
        assert!(automation.process(&settings, &first, now).is_none());
        if let EventKind::Chat(message) = &mut first.kind {
            message.text = "!saluda dos".into();
            message.id = MessageId::new("2");
        }
        if let EventKind::Chat(message) = &mut first.kind {
            message.author_id = Some("42".into());
        }
        // El origen Twitch no está en la lista permitida.
        assert!(
            automation
                .process(&settings, &first, now + Duration::from_secs(6))
                .is_none()
        );
        settings.commands[0].platforms.clear();
        if let EventKind::Chat(message) = &mut first.kind {
            message.text = "!saluda dos alternativo".into();
            message.id = MessageId::new("allowed");
        }
        assert!(
            automation
                .process(&settings, &first, now + Duration::from_secs(6))
                .is_some()
        );
        let mut second = event("!saluda tres", "3", ChatRoles::default());
        if let EventKind::Chat(message) = &mut second.kind {
            message.is_self = Some(false);
        }
        assert!(
            automation
                .process(&settings, &second, now + Duration::from_secs(7))
                .is_none()
        );
        assert!(
            automation
                .process(&settings, &second, now + Duration::from_secs(12))
                .is_some()
        );
    }

    #[test]
    fn respuesta_identica_tiene_enfriamiento_por_plataforma() {
        let now = Instant::now();
        let mut settings = ChatSettings {
            duplicate_window_seconds: 1,
            ..ChatSettings::default()
        };
        settings.commands.push(command(MinimumChatRole::Everyone));
        settings.commands[0].response = "respuesta fija".into();
        let mut automation = ChatAutomation::default();
        let mut first = event("!saluda", "a", ChatRoles::default());
        if let EventKind::Chat(message) = &mut first.kind {
            message.is_self = Some(false);
        }
        assert!(automation.process(&settings, &first, now).is_some());
        let mut second = event("!saluda", "b", ChatRoles::default());
        if let EventKind::Chat(message) = &mut second.kind {
            message.is_self = Some(false);
            message.author_id = Some("43".into());
        }
        assert!(
            automation
                .process(&settings, &second, now + Duration::from_secs(2))
                .is_none()
        );
        assert!(
            automation
                .process(&settings, &second, now + Duration::from_secs(6))
                .is_some()
        );
    }
}
