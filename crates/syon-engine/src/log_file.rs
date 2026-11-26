use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Destino (`target`) con el que se escribe la línea de marca de cada arranque.
pub(crate) const MARKER_TARGET: &str = "syon::arranque";
/// Lo que sigue a la marca de tiempo en una línea de marca: el nivel y el destino tal como los
/// escribe `logging::format_record`. Una prueba de `logging` comprueba que ambos coinciden.
const MARKER_HEADER: &str = "INFO  [syon::arranque] ";
const ROTATED_SUFFIX: &str = ".1";
const TEMP_SUFFIX: &str = ".tmp";

/// Archivo de registro con un tope de tamaño: al llenarse pasa a `<nombre>.1`, que sustituye al
/// anterior, y se empieza otro. Al abrirlo se descartan las ejecuciones más antiguas.
///
/// Cada escritura va directa al archivo, sin búfer, para que un pánico o un cierre brusco no
/// pierda las últimas líneas. Pensado para `env_logger`, que entrega cada registro entero en una
/// sola escritura.
pub(crate) struct RotatingFile {
    path: PathBuf,
    rotated: PathBuf,
    max_bytes: u64,
    file: File,
    written: u64,
}

impl RotatingFile {
    /// Abre `path` para añadir al final y crea su carpeta si falta. Antes conserva solo las
    /// `keep_runs` ejecuciones anteriores más recientes entre `path` y `<path>.1`.
    pub(crate) fn open(path: &Path, max_bytes: u64, keep_runs: usize) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let rotated = with_suffix(path, ROTATED_SUFFIX);
        prune_runs(path, &rotated, keep_runs)?;
        let file = open_append(path)?;
        let written = file.metadata()?.len();
        Ok(Self {
            path: path.to_path_buf(),
            rotated,
            max_bytes,
            file,
            written,
        })
    }

    /// Mueve el archivo actual a `<nombre>.1` y abre uno vacío.
    fn rotate(&mut self) -> io::Result<()> {
        fs::rename(&self.path, &self.rotated)?;
        self.file = open_append(&self.path)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Un registro mayor que el tope en un archivo vacío se escribe entero, sin rotar en bucle.
        if self.written > 0 && self.written + buf.len() as u64 > self.max_bytes {
            // Si el sistema no deja renombrar (un antivirus, un lector abierto), se sigue
            // escribiendo en el archivo actual: perder el registro sería peor que pasarse del tope.
            let _ = self.rotate();
        }
        let written = self.file.write(buf)?;
        self.written += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Indica si `line` es la marca de arranque que escribe el registro: empieza sin sangría y, tras
/// la marca de tiempo, trae el nivel y el destino de la marca. Las líneas de continuación de un
/// mensaje llevan sangría, así que un texto de chat no puede imitarla.
pub(crate) fn is_run_marker(line: &[u8]) -> bool {
    if line.first().is_none_or(u8::is_ascii_whitespace) {
        return false;
    }
    line.iter()
        .position(|&b| b == b' ')
        .is_some_and(|space| line[space + 1..].starts_with(MARKER_HEADER.as_bytes()))
}

/// Deja solo las `keep` ejecuciones más recientes de la historia formada por `rotated` (la más
/// antigua) y `current`. Una ejecución empieza en una línea de marca; lo que haya antes de la
/// primera marca es el resto de una ejecución cuya marca ya se descartó y cuenta como una más.
fn prune_runs(current: &Path, rotated: &Path, keep: usize) -> io::Result<()> {
    let older = read_or_empty(rotated)?;
    let newer = read_or_empty(current)?;
    let mut starts: Vec<(bool, usize)> = Vec::new(); // (en `newer`, desplazamiento)
    for (in_newer, bytes) in [(false, &older), (true, &newer)] {
        let markers = marker_offsets(bytes);
        if starts.is_empty() && !bytes.is_empty() && markers.first() != Some(&0) {
            starts.push((in_newer, 0));
        }
        starts.extend(markers.into_iter().map(|offset| (in_newer, offset)));
    }
    if starts.len() <= keep {
        return Ok(());
    }
    // Con `keep == 0` no queda nada: el corte cae al final del archivo actual.
    let (in_newer, offset) = match starts.len().checked_sub(keep).and_then(|i| starts.get(i)) {
        Some(&cut) => cut,
        None => (true, newer.len()),
    };
    if in_newer {
        remove_if_exists(rotated)?;
        if offset > 0 {
            replace_contents(current, &newer[offset..])?;
        }
    } else {
        replace_contents(rotated, &older[offset..])?;
    }
    Ok(())
}

fn marker_offsets(bytes: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut position = 0;
    for line in bytes.split_inclusive(|&b| b == b'\n') {
        if is_run_marker(line) {
            offsets.push(position);
        }
        position += line.len();
    }
    offsets
}

fn open_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn read_or_empty(path: &Path) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        other => other,
    }
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Reemplaza el contenido de `path` escribiendo antes un temporal en la misma carpeta.
fn replace_contents(path: &Path, contents: &[u8]) -> io::Result<()> {
    let temp = with_suffix(path, TEMP_SUFFIX);
    fs::write(&temp, contents)?;
    fs::rename(&temp, path)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Carpeta temporal única que se borra al soltarla (sin dependencias de prueba).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("syon-logfile-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn log(&self) -> PathBuf {
            self.0.join("syon.log")
        }

        fn rotated(&self) -> PathBuf {
            self.0.join("syon.log.1")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn marker(run: u32) -> String {
        format!(
            "2026-11-25T10:00:0{run}.000Z INFO  [syon::arranque] logging: syon 0.1.0 iniciado\n"
        )
    }

    /// Una ejecución de `lines` líneas de relleno que empieza con su marca.
    fn run(run: u32, lines: u32) -> String {
        let body: String = (0..lines)
            .map(|n| format!("2026-11-25T10:00:0{run}.100Z DEBUG [x] ejecucion {run} linea {n}\n"))
            .collect();
        marker(run) + &body
    }

    fn runs_in(path: &Path) -> Vec<u32> {
        let text = fs::read_to_string(path).unwrap_or_default();
        text.lines()
            .filter(|line| is_run_marker(line.as_bytes()))
            .map(|line| line[18..19].parse().unwrap())
            .collect()
    }

    #[test]
    fn crea_la_carpeta_y_anade_al_final_de_lo_que_hubiera() {
        let dir = TempDir::new();
        let log = dir.0.join("datos").join("syon.log");
        let mut file = RotatingFile::open(&log, 1024, 4).unwrap();
        file.write_all(b"uno\n").unwrap();
        drop(file);
        let mut file = RotatingFile::open(&log, 1024, 4).unwrap();
        file.write_all(b"dos\n").unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap(), "uno\ndos\n");
    }

    #[test]
    fn al_superar_el_tope_pasa_a_punto_uno_y_empieza_otro() {
        let dir = TempDir::new();
        let mut file = RotatingFile::open(&dir.log(), 10, 4).unwrap();
        file.write_all(b"aaaaaa\n").unwrap();
        file.write_all(b"bbbbbb\n").unwrap();
        assert_eq!(fs::read_to_string(dir.rotated()).unwrap(), "aaaaaa\n");
        assert_eq!(fs::read_to_string(dir.log()).unwrap(), "bbbbbb\n");
    }

    #[test]
    fn la_segunda_rotacion_sustituye_a_punto_uno() {
        let dir = TempDir::new();
        let mut file = RotatingFile::open(&dir.log(), 10, 4).unwrap();
        for text in ["aaaaaa\n", "bbbbbb\n", "cccccc\n"] {
            file.write_all(text.as_bytes()).unwrap();
        }
        assert_eq!(fs::read_to_string(dir.rotated()).unwrap(), "bbbbbb\n");
        assert_eq!(fs::read_to_string(dir.log()).unwrap(), "cccccc\n");
    }

    #[test]
    fn un_registro_mayor_que_el_tope_se_escribe_entero_sin_rotar_en_vacio() {
        let dir = TempDir::new();
        let mut file = RotatingFile::open(&dir.log(), 4, 4).unwrap();
        file.write_all(b"registro muy largo\n").unwrap();
        assert_eq!(
            fs::read_to_string(dir.log()).unwrap(),
            "registro muy largo\n"
        );
        assert!(!dir.rotated().exists());
    }

    #[test]
    fn un_archivo_ya_lleno_al_abrir_rota_en_la_primera_escritura() {
        let dir = TempDir::new();
        fs::write(dir.log(), "viejo viejo\n").unwrap();
        let mut file = RotatingFile::open(&dir.log(), 10, 4).unwrap();
        file.write_all(b"nuevo\n").unwrap();
        assert_eq!(fs::read_to_string(dir.rotated()).unwrap(), "viejo viejo\n");
        assert_eq!(fs::read_to_string(dir.log()).unwrap(), "nuevo\n");
    }

    #[test]
    fn la_marca_se_reconoce_solo_en_la_primera_linea_de_un_registro() {
        assert!(is_run_marker(marker(1).as_bytes()));
        assert!(!is_run_marker(
            b"2026-11-25T10:00:01.000Z DEBUG [x] chat: INFO  [syon::arranque] hola\n"
        ));
        assert!(!is_run_marker(
            b"    2026-11-25T10:00:01.000Z INFO  [syon::arranque] logging: syon\n"
        ));
        assert!(!is_run_marker(b""));
        assert!(!is_run_marker(b"\n"));
    }

    #[test]
    fn con_menos_ejecuciones_de_las_permitidas_no_toca_nada() {
        let dir = TempDir::new();
        let text = run(1, 2) + &run(2, 2);
        fs::write(dir.log(), &text).unwrap();
        drop(RotatingFile::open(&dir.log(), 1024, 4).unwrap());
        assert_eq!(fs::read_to_string(dir.log()).unwrap(), text);
    }

    #[test]
    fn descarta_las_ejecuciones_mas_antiguas_de_un_solo_archivo() {
        let dir = TempDir::new();
        let text: String = (1..=6).map(|n| run(n, 1)).collect();
        fs::write(dir.log(), text).unwrap();
        drop(RotatingFile::open(&dir.log(), 1024, 4).unwrap());
        assert_eq!(runs_in(&dir.log()), [3, 4, 5, 6]);
        assert!(!dir.rotated().exists());
    }

    #[test]
    fn la_historia_cuenta_los_dos_archivos_y_recorta_el_mas_antiguo() {
        let dir = TempDir::new();
        fs::write(dir.rotated(), run(1, 1) + &run(2, 1) + &run(3, 1)).unwrap();
        fs::write(dir.log(), run(4, 1) + &run(5, 1)).unwrap();
        drop(RotatingFile::open(&dir.log(), 1024, 4).unwrap());
        assert_eq!(runs_in(&dir.rotated()), [2, 3]);
        assert_eq!(runs_in(&dir.log()), [4, 5]);
    }

    #[test]
    fn si_el_corte_cae_en_el_archivo_actual_se_borra_punto_uno() {
        let dir = TempDir::new();
        fs::write(dir.rotated(), run(1, 1) + &run(2, 1)).unwrap();
        fs::write(
            dir.log(),
            run(3, 1) + &run(4, 1) + &run(5, 1) + &run(6, 1) + &run(7, 1),
        )
        .unwrap();
        drop(RotatingFile::open(&dir.log(), 1024, 4).unwrap());
        assert!(!dir.rotated().exists());
        assert_eq!(runs_in(&dir.log()), [4, 5, 6, 7]);
    }

    #[test]
    fn el_resto_de_una_ejecucion_sin_marca_cuenta_como_una_ejecucion() {
        let dir = TempDir::new();
        let fragment =
            "2026-11-25T09:00:00.100Z WARN  [x] cola de una ejecucion cuya marca se perdio\n";
        let text = format!(
            "{fragment}{}",
            (1..=4).map(|n| run(n, 1)).collect::<String>()
        );
        fs::write(dir.log(), text).unwrap();
        drop(RotatingFile::open(&dir.log(), 1024, 4).unwrap());
        assert_eq!(runs_in(&dir.log()), [1, 2, 3, 4]);
        drop(RotatingFile::open(&dir.log(), 1024, 3).unwrap());
        assert_eq!(runs_in(&dir.log()), [2, 3, 4]);
        assert!(
            !fs::read_to_string(dir.log())
                .unwrap()
                .contains("cola de una")
        );
    }

    #[test]
    fn una_ejecucion_partida_entre_los_dos_archivos_cuenta_una_sola_vez() {
        let dir = TempDir::new();
        // La ejecución 2 empezó en `.1` y siguió en el archivo actual, sin marca nueva.
        fs::write(dir.rotated(), run(1, 1) + &run(2, 1)).unwrap();
        fs::write(
            dir.log(),
            "2026-11-25T10:00:02.200Z DEBUG [x] sigue la ejecucion 2\n",
        )
        .unwrap();
        drop(RotatingFile::open(&dir.log(), 1024, 1).unwrap());
        assert_eq!(runs_in(&dir.rotated()), [2]);
        assert!(
            fs::read_to_string(dir.log())
                .unwrap()
                .contains("sigue la ejecucion 2")
        );
    }

    #[test]
    fn con_cero_ejecuciones_conservadas_el_archivo_queda_vacio() {
        let dir = TempDir::new();
        fs::write(dir.rotated(), run(1, 1)).unwrap();
        fs::write(dir.log(), run(2, 1)).unwrap();
        drop(RotatingFile::open(&dir.log(), 1024, 0).unwrap());
        assert!(!dir.rotated().exists());
        assert_eq!(fs::read_to_string(dir.log()).unwrap(), "");
    }
}
