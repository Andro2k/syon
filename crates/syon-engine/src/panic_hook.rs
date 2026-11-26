use std::{
    backtrace::{Backtrace, BacktraceStatus},
    panic::{self, PanicHookInfo},
    thread,
};

/// Hace que cada pánico quede en el registro (mensaje, hilo, lugar y, si `RUST_BACKTRACE` lo
/// habilita, el backtrace) antes de que actúe el hook anterior, que sigue imprimiéndolo en la
/// consola como siempre. Reemplaza el hook global del proceso: se instala una sola vez.
pub(crate) fn install() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        log::error!("{}", describe(info));
        log::logger().flush();
        previous(info);
    }));
}

fn describe(info: &PanicHookInfo<'_>) -> String {
    let thread = thread::current();
    let location = info.location().map(ToString::to_string);
    let backtrace = Backtrace::capture();
    format_panic(
        thread.name().unwrap_or("<sin nombre>"),
        location.as_deref(),
        info.payload_as_str().unwrap_or("<sin mensaje>"),
        (backtrace.status() == BacktraceStatus::Captured).then(|| backtrace.to_string()),
    )
}

fn format_panic(
    thread: &str,
    location: Option<&str>,
    message: &str,
    backtrace: Option<String>,
) -> String {
    let location = location.unwrap_or("lugar desconocido");
    let mut text = format!("pánico: el hilo '{thread}' falló en {location}: {message}");
    if let Some(backtrace) = backtrace {
        text.push('\n');
        text.push_str(backtrace.trim_end());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_mensaje_nombra_el_hilo_el_lugar_y_la_causa() {
        assert_eq!(
            format_panic("main", Some("src/lib.rs:7:5"), "boom", None),
            "pánico: el hilo 'main' falló en src/lib.rs:7:5: boom"
        );
    }

    #[test]
    fn sin_lugar_lo_dice_y_con_backtrace_lo_añade_en_lineas_aparte() {
        let text = format_panic("tts", None, "boom", Some("   0: a\n   1: b\n".to_owned()));
        assert_eq!(
            text,
            "pánico: el hilo 'tts' falló en lugar desconocido: boom\n   0: a\n   1: b"
        );
    }
}
