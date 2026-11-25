//! Comprobaciones de la documentación: enlaces relativos, índice de ADR y
//! README de cada crate.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Devuelve la lista de problemas encontrados; vacía si todo está en orden.
pub fn check(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    check_links(root, &mut problems);
    check_adr_index(root, &mut problems);
    check_crate_readmes(root, &mut problems);
    problems
}

fn check_links(root: &Path, problems: &mut Vec<String>) {
    let mut files = vec![root.join("README.md"), root.join("AGENTS.md")];
    collect_markdown(&root.join("docs"), &mut files);
    for file in files.iter().filter(|f| f.is_file()) {
        let Ok(text) = fs::read_to_string(file) else {
            problems.push(format!("no se pudo leer {}", display(root, file)));
            continue;
        };
        let dir = file.parent().unwrap_or(root);
        for link in extract_links(&text) {
            if !dir.join(&link).exists() {
                problems.push(format!("enlace roto en {}: {link}", display(root, file)));
            }
        }
    }
}

fn check_adr_index(root: &Path, problems: &mut Vec<String>) {
    let dir = root.join("docs/adr");
    let Ok(index) = fs::read_to_string(dir.join("README.md")) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_adr = name.ends_with(".md")
            && !name.starts_with("0000-")
            && name[..4].bytes().all(|b| b.is_ascii_digit());
        if is_adr && !index.contains(&name) {
            problems.push(format!("el ADR {name} no está en docs/adr/README.md"));
        }
    }
}

fn check_crate_readmes(root: &Path, problems: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root.join("crates")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !path.join("README.md").is_file() {
            problems.push(format!("falta README.md en {}", display(root, &path)));
        }
    }
}

fn collect_markdown(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Enlaces relativos de un Markdown, sin ancla, ignorando URL, anclas puras y
/// bloques de código.
fn extract_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            let Some(end) = after.find(')') else { break };
            let target = after[..end].split_whitespace().next().unwrap_or("");
            let path = target.split('#').next().unwrap_or("");
            let is_external = target.contains("://") || target.starts_with("mailto:");
            if !path.is_empty() && !is_external {
                links.push(path.to_owned());
            }
            rest = &after[end..];
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_root() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "syon_docs_check_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn extrae_solo_enlaces_relativos() {
        let text = "[a](x.md) [b](https://e.com) [c](#ancla) [d](y.md#sec)\n```\n[e](z.md)\n```";
        assert_eq!(extract_links(text), ["x.md", "y.md"]);
    }

    #[test]
    fn documentacion_correcta_no_da_problemas() {
        let root = temp_root();
        write(&root, "README.md", "[v](docs/vision.md)");
        write(&root, "docs/vision.md", "texto");
        write(&root, "docs/adr/README.md", "[1](0001-uno.md)");
        write(&root, "docs/adr/0001-uno.md", "adr");
        write(&root, "docs/adr/0000-plantilla.md", "plantilla");
        assert!(check(&root).is_empty());
    }

    #[test]
    fn comprueba_los_enlaces_de_agents_md() {
        let root = temp_root();
        write(&root, "AGENTS.md", "[roto](docs/nada.md)");
        assert_eq!(check(&root).len(), 1);
    }

    #[test]
    fn detecta_enlace_roto_adr_sin_indice_y_crate_sin_readme() {
        let root = temp_root();
        write(&root, "README.md", "[roto](docs/nada.md)");
        write(&root, "docs/adr/README.md", "vacío");
        write(&root, "docs/adr/0002-dos.md", "adr");
        write(&root, "crates/syon-domain/Cargo.toml", "");
        let problems = check(&root);
        assert_eq!(problems.len(), 3, "{problems:?}");
    }
}
