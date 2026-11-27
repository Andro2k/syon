//! Las credenciales de la aplicación se incrustan al compilar con `option_env!` (ADR-0018). Sin
//! esto, Cargo reutilizaría un binario compilado con otro valor de estas variables.

fn main() {
    for name in [
        "SYON_KICK_CLIENT_ID",
        "SYON_KICK_CLIENT_SECRET",
        "SYON_TWITCH_CLIENT_ID",
        "SYON_TWITCH_CLIENT_SECRET",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
}
