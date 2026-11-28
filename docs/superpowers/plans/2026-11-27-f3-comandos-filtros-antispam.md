# F3 Commands, Filters and Anti-Spam Implementation Plan

> **For agentic workers:** Use this plan task by task. The repository requires one green, atomic `git format-patch` per step.

**Goal:** Run safe chat commands from Kick and Twitch events, apply configured chat filters and prevent repeated responses.

**Architecture:** `syon-domain` carries platform identity and normalized role flags. `syon-app` owns serializable-by-adapter settings models; `syon-storage-toml` parses and validates them. `syon-engine` owns the bounded event consumer, command dispatch, filtering, duplicate tracking, rate limiting and asynchronous sends. `syon` keeps its existing event display and manual chat input.

**Tech Stack:** Rust 2024, Tokio already in the workspace, `std::time::Instant`, TOML adapter already present. No direct dependencies are added.

**Spec:** `docs/designs/005-comandos-filtros-antispam.md`

## Global Constraints

- `syon-domain` has no external or workspace dependencies; `syon-app` only depends on `syon-domain`.
- Adapter payloads are tolerant; unknown Kick roles remain false and unknown authors are not automated.
- Own or unclassified chat events never dispatch commands.
- The event bus stays nonblocking; automation uses its own bounded queue.
- No chat text, user name, message id, access token or client secret enters ordinary logs.
- Every patch updates this front's row in `docs/estado.md`; final closure updates parity and a walkthrough.

## Review Focus

1. Own echo arrives before send response: register outgoing text before network I/O and assert one matching echo is excluded.
2. Missing author id/roles: assert unknown identity and false roles cannot authorize commands.
3. Duplicate alias/name in TOML: assert load fails without overwriting the invalid source file.
4. Repeated matching messages: assert duplicate detection, five-second per-user command cooldown and one-response-per-second platform limit.
5. Full event queue: assert `publish` remains nonblocking and the subscriber's dropped counter increases.

---

### Task 1: Accept design and mark the implementation front

**Files:** `docs/designs/005-comandos-filtros-antispam.md`, `docs/designs/README.md`, `docs/estado.md` (F3 part 2 row only).

- [ ] Mark the design accepted by the user's implementation request; record confirmed Twitch/Kick echo behavior and official role field sources.
- [ ] Keep Kick Pusher feed fields optional and tolerant; only map documented `sender.user_id` and known badge `type` values, defaulting unknown fields to no roles.
- [ ] Record implementation as in progress and retain unresolved live-field risk for Kick.
- [ ] Run `cargo xtask docs-check` and `git diff --check`.

### Task 2: Add chat identity and role data

**Files:** `crates/syon-domain/src/event.rs`, `crates/syon-domain/src/lib.rs`, `crates/syon-platform-twitch/src/events.rs`, `crates/syon-platform-twitch/src/connector.rs`, `crates/syon-platform-kick/src/events.rs`, `crates/syon-platform-kick/src/connector.rs`, and existing `ChatMessage` constructors in tests.

**Interface:** `ChatRoles { broadcaster, moderator, vip, subscriber }`; `ChatMessage` gains `author_id: Option<String>`, `roles: ChatRoles` and `is_self: Option<bool>`.

- [ ] Add tests for Twitch badge-to-role mapping, broadcaster id, author id and own-user comparison; add Kick tests for optional documented user id and known badges, and unknown values.
- [ ] Run each affected adapter test and observe the expected compile/assertion failure.
- [ ] Implement the model and tolerant translators; Twitch receives its validated `user_id` from the connector. Kick never guesses a missing role or identity.
- [ ] Run domain and both adapter test suites.

### Task 3: Add application settings models

**Files:** `crates/syon-app/src/settings.rs`, `crates/syon-app/src/lib.rs`.

**Interface:** `ChatSettings`, `ChatCommand`, `ChatUserFilter`, `MinimumChatRole`, `ChatSettings::default()`; defaults use `!`, no configured commands, 5-second command cooldown, one send per second per platform and a 10-second duplicate window.

- [ ] Add tests for defaults, valid command fields and exact role authorization hierarchy.
- [ ] Run `cargo test -p syon-app` and observe the expected failure.
- [ ] Add the types and a `chat` field to `Settings`; empty `commands` keeps automation inert by default.
- [ ] Run `cargo test -p syon-app`.

### Task 4: Persist and validate `[chat]`

**Files:** `crates/syon-storage-toml/src/format.rs`, `crates/syon-storage-toml/src/store.rs`.

- [ ] Add round-trip tests for prefix, commands, aliases, platform restrictions, destinations, role, ignored users, blocked phrases, required phrases and limits.
- [ ] Add invalid-TOML-model tests for empty/long prefix, empty command name/response, duplicate names or aliases, unknown platforms/roles and zero or excessive limits.
- [ ] Run storage tests and observe the missing section behavior.
- [ ] Implement serde-only adapter structs and explicit validation with Spanish errors; missing `[chat]` uses defaults and old settings remain readable.
- [ ] Run `cargo test -p syon-storage-toml`.

### Task 5: Implement command parsing and policy logic

**Files:** `crates/syon-engine/src/commands.rs` (new), `crates/syon-engine/src/lib.rs`, `crates/syon-engine/src/engine.rs` as needed.

- [ ] Add pure tests for prefix parsing, Unicode whitespace arguments, alias matching, literal substitutions `{user}`, `{platform}`, `{args}`, platform allow-list, role levels, filters and cooldowns with a supplied `Instant`.
- [ ] Run `cargo test -p syon-engine commands` and observe the expected missing-module failure.
- [ ] Implement a single-message decision function with no I/O; ignore own/unknown messages before filters and parsing; unknown commands remain silent.
- [ ] Bound/prune recent ids and content fingerprints; use 10-second duplicate window, 5-second per-user-command cooldown and 1-second platform send interval.
- [ ] Run `cargo test -p syon-engine commands`.

### Task 6: Wire the bounded consumer and asynchronous sender

**Files:** `crates/syon-engine/src/chat.rs`, `crates/syon-engine/src/engine.rs`, `crates/syon-engine/src/event_bus.rs` tests as needed.

- [ ] Add a test proving `ChatHandle::send_async` works from a Tokio task.
- [ ] Add tests proving own message ids and pre-registered exact outgoing text are consumed once, send errors remove pending text, unknown identity does not dispatch, responses target the source platform by default, and full consumer queues do not block publication.
- [ ] Run focused tests and observe expected failures.
- [ ] Register the automation subscriber before platform startup, spawn a sequential worker on the existing runtime and await `send_async` outside the publisher path.
- [ ] Keep own-message correlation bounded, with expiry; cache successful returned message ids and register normalized outbound text before starting each send.
- [ ] Run engine tests, then the full workspace.

### Task 7: Close F3 part 2

**Files:** `docs/designs/005-comandos-filtros-antispam.md`, `docs/designs/README.md`, `docs/estado.md` (F3 part 2 row only), `docs/requirements/paridad.md`, `docs/walkthroughs/WT-0.4.0_01.md`.

- [ ] Record implemented behavior and remaining live-confirmation risk for Kick; update parity and the walkthrough with actual patch names/files and validation output.
- [ ] Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo xtask docs-check` and `cargo xtask arch-check`.
- [ ] Export the atomic commits as consecutively numbered format-patches, apply them with `git am` to a fresh clone of current `origin/master`, and repeat validation with a separate target directory.
