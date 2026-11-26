//! `cargo xtask arch-check`: hace cumplir la dirección de dependencias entre crates
//! (diseño 001, sección 10; [AGENTS.md](../../AGENTS.md)). Lee `cargo metadata` y comprueba las
//! reglas de la tabla [`RULES`].

use crate::json::{self, Json};
use std::{collections::BTreeMap, path::Path, process::Command};

/// Papel de un crate del workspace en la arquitectura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Domain,
    App,
    Engine,
    Adapter,
    Binary,
    Ui,
    /// Herramientas del repositorio (`xtask`): quedan fuera de las reglas.
    Tool,
}

/// Prefijos de los nombres de los adaptadores (ADR-0003). Un crate nuevo cuyo nombre no encaje
/// aquí ni en [`role_of`] se rechaza hasta que se decida su papel y se añada a la tabla.
const ADAPTER_PREFIXES: &[&str] = &[
    "syon-storage-",
    "syon-platform-",
    "syon-tts-",
    "syon-music-",
    "syon-webhook-",
];

/// Papel de un crate según su nombre; `None` si la tabla no lo conoce.
pub fn role_of(name: &str) -> Option<Role> {
    match name {
        "syon-domain" => Some(Role::Domain),
        "syon-app" => Some(Role::App),
        "syon-engine" => Some(Role::Engine),
        "syon" => Some(Role::Binary),
        "syon-ui" => Some(Role::Ui),
        "xtask" => Some(Role::Tool),
        _ if ADAPTER_PREFIXES.iter().any(|p| name.starts_with(p)) => Some(Role::Adapter),
        _ => None,
    }
}

/// Una dependencia declarada por un crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    /// `normal`, `dev` o `build`.
    pub kind: String,
    /// Papel si es un crate del workspace; `None` si es externa.
    pub role: Option<Role>,
}

/// Un crate del workspace con sus dependencias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub role: Option<Role>,
    pub deps: Vec<Dep>,
}

/// Una regla: `violates(origen, destino)` es verdadero si la arista `origen → destino` la rompe.
pub struct Rule {
    pub id: u8,
    pub text: &'static str,
    pub violates: fn(from: Role, to: Option<Role>) -> bool,
}

/// Reglas de la sección 10 del diseño 001. `to == None` es una dependencia externa.
pub const RULES: &[Rule] = &[
    Rule {
        id: 1,
        text: "syon-domain no tiene dependencias externas ni de otros crates del workspace",
        violates: |from, _| from == Role::Domain,
    },
    Rule {
        id: 2,
        text: "syon-app solo depende de syon-domain",
        violates: |from, to| from == Role::App && to != Some(Role::Domain),
    },
    Rule {
        id: 3,
        text: "un adaptador no depende de otro adaptador ni de syon-engine",
        violates: |from, to| {
            from == Role::Adapter && matches!(to, Some(Role::Adapter | Role::Engine))
        },
    },
    Rule {
        id: 4,
        text: "solo el binario y la UI dependen de syon-engine",
        violates: |from, to| to == Some(Role::Engine) && !matches!(from, Role::Binary | Role::Ui),
    },
    Rule {
        id: 5,
        text: "solo syon-engine y el binario dependen de un adaptador",
        violates: |from, to| {
            to == Some(Role::Adapter) && !matches!(from, Role::Engine | Role::Binary)
        },
    },
];

/// Devuelve las violaciones; vacía si la arquitectura se respeta. Cuentan todas las clases de
/// dependencia (normales, de desarrollo y de compilación): una arista de prueba también acopla.
pub fn check(members: &[Member]) -> Vec<String> {
    let mut problems = Vec::new();
    for member in members {
        let Some(from) = member.role else {
            problems.push(format!(
                "el crate `{}` no tiene papel en la tabla de xtask/src/arch_check.rs; añádelo (role_of) o renómbralo",
                member.name
            ));
            continue;
        };
        if from == Role::Tool {
            continue;
        }
        for dep in &member.deps {
            for rule in RULES.iter().filter(|r| (r.violates)(from, dep.role)) {
                problems.push(format!(
                    "regla {} ({}): {} → {} [{}]",
                    rule.id, rule.text, member.name, dep.name, dep.kind
                ));
            }
        }
    }
    problems
}

/// Ejecuta `cargo metadata` en `root` y devuelve los crates del workspace.
pub fn load_members(root: &Path) -> Result<Vec<Member>, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("no se pudo ejecutar cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata falló: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout).map_err(|_| "cargo metadata no devolvió UTF-8")?;
    members_from_metadata(&text)
}

/// Extrae los crates del workspace de la salida de `cargo metadata --no-deps`.
pub fn members_from_metadata(text: &str) -> Result<Vec<Member>, String> {
    let metadata = json::parse(text)?;
    let packages = metadata
        .get("packages")
        .and_then(Json::as_array)
        .ok_or("cargo metadata no trae `packages`")?;
    let names: Vec<&str> = packages
        .iter()
        .filter_map(|p| p.get("name").and_then(Json::as_str))
        .collect();
    let mut members = BTreeMap::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(Json::as_str)
            .ok_or("un paquete sin nombre")?;
        let deps = package
            .get("dependencies")
            .and_then(Json::as_array)
            .ok_or_else(|| format!("el paquete `{name}` no trae `dependencies`"))?
            .iter()
            .map(|dep| dependency(dep, &names))
            .collect::<Result<Vec<_>, _>>()?;
        members.insert(
            name.to_owned(),
            Member {
                name: name.to_owned(),
                role: role_of(name),
                deps,
            },
        );
    }
    Ok(members.into_values().collect())
}

fn dependency(dep: &Json, workspace_names: &[&str]) -> Result<Dep, String> {
    let name = dep
        .get("name")
        .and_then(Json::as_str)
        .ok_or("una dependencia sin nombre")?;
    let kind = dep.get("kind").and_then(Json::as_str).unwrap_or("normal");
    let in_workspace = workspace_names.contains(&name);
    Ok(Dep {
        name: name.to_owned(),
        kind: kind.to_owned(),
        // Una dependencia de un crate del workspace sin papel se trata como externa; el propio
        // crate ya se rechaza por no tener papel.
        role: in_workspace.then(|| role_of(name)).flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep(name: &str) -> Dep {
        Dep {
            name: name.to_owned(),
            kind: "normal".to_owned(),
            role: role_of(name),
        }
    }

    fn external(name: &str) -> Dep {
        Dep {
            role: None,
            ..dep(name)
        }
    }

    fn member(name: &str, deps: Vec<Dep>) -> Member {
        Member {
            name: name.to_owned(),
            role: role_of(name),
            deps,
        }
    }

    /// El workspace actual: debe cumplir todas las reglas.
    fn valid_workspace() -> Vec<Member> {
        vec![
            member("syon-domain", vec![]),
            member("syon-app", vec![dep("syon-domain")]),
            member(
                "syon-storage-toml",
                vec![dep("syon-app"), external("toml"), external("serde")],
            ),
            member(
                "syon-engine",
                vec![
                    dep("syon-app"),
                    dep("syon-domain"),
                    dep("syon-storage-toml"),
                    external("log"),
                ],
            ),
            member("syon", vec![dep("syon-engine"), dep("syon-ui")]),
            member("syon-ui", vec![dep("syon-engine"), dep("syon-app")]),
            member("xtask", vec![]),
        ]
    }

    fn rules_broken(members: &[Member]) -> Vec<u8> {
        check(members)
            .iter()
            .map(|p| {
                p.strip_prefix("regla ")
                    .and_then(|r| r[..1].parse().ok())
                    .unwrap_or(0)
            })
            .collect()
    }

    #[test]
    fn una_arquitectura_valida_no_da_problemas() {
        assert_eq!(check(&valid_workspace()), Vec::<String>::new());
    }

    /// Cambia un crate del workspace válido y devuelve las reglas que se rompen.
    fn with(name: &str, deps: Vec<Dep>) -> Vec<u8> {
        let mut members = valid_workspace();
        members.iter_mut().find(|m| m.name == name).unwrap().deps = deps;
        rules_broken(&members)
    }

    #[test]
    fn regla_1_domain_con_una_dependencia_externa() {
        assert_eq!(with("syon-domain", vec![external("serde")]), [1]);
    }

    #[test]
    fn regla_1_domain_con_otro_crate_del_workspace() {
        assert_eq!(with("syon-domain", vec![dep("syon-app")]), [1]);
    }

    #[test]
    fn regla_2_app_con_una_dependencia_externa() {
        assert_eq!(
            with("syon-app", vec![dep("syon-domain"), external("serde")]),
            [2]
        );
    }

    #[test]
    fn regla_2_app_con_otro_crate_del_workspace() {
        assert_eq!(
            with(
                "syon-app",
                vec![dep("syon-domain"), dep("syon-storage-toml")]
            ),
            [2, 5]
        );
    }

    #[test]
    fn regla_3_adaptador_que_depende_de_otro_adaptador() {
        let mut members = valid_workspace();
        members.push(member(
            "syon-platform-kick",
            vec![dep("syon-app"), dep("syon-storage-toml")],
        ));
        assert_eq!(rules_broken(&members), [3, 5]);
    }

    #[test]
    fn regla_3_adaptador_que_depende_del_engine() {
        assert_eq!(
            with(
                "syon-storage-toml",
                vec![dep("syon-app"), dep("syon-engine")]
            ),
            [3, 4]
        );
    }

    #[test]
    fn regla_4_un_crate_cualquiera_que_depende_del_engine() {
        assert_eq!(
            with("syon-app", vec![dep("syon-domain"), dep("syon-engine")]),
            [2, 4]
        );
    }

    #[test]
    fn regla_5_un_crate_que_no_es_el_engine_depende_de_un_adaptador() {
        assert_eq!(
            with(
                "syon-ui",
                vec![dep("syon-engine"), dep("syon-storage-toml")]
            ),
            [5]
        );
    }

    #[test]
    fn las_dependencias_de_desarrollo_tambien_cuentan() {
        let mut dev = external("tempfile");
        dev.kind = "dev".to_owned();
        let problems = check(&{
            let mut members = valid_workspace();
            members[0].deps.push(dev);
            members
        });
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].contains("syon-domain → tempfile [dev]"),
            "{problems:?}"
        );
    }

    #[test]
    fn el_binario_puede_depender_del_engine_y_de_un_adaptador() {
        assert_eq!(
            with("syon", vec![dep("syon-engine"), dep("syon-storage-toml")]),
            Vec::<u8>::new()
        );
    }

    #[test]
    fn un_crate_sin_papel_se_rechaza() {
        let mut members = valid_workspace();
        members.push(member("syon-misterio", vec![]));
        let problems = check(&members);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("syon-misterio") && problems[0].contains("papel"));
    }

    #[test]
    fn xtask_queda_fuera_de_las_reglas() {
        assert_eq!(
            with("xtask", vec![dep("syon-engine"), external("serde")]),
            Vec::<u8>::new()
        );
    }

    #[test]
    fn cada_regla_tiene_un_numero_unico_y_un_texto() {
        let ids: Vec<u8> = RULES.iter().map(|r| r.id).collect();
        assert_eq!(ids, [1, 2, 3, 4, 5]);
        assert!(RULES.iter().all(|r| !r.text.is_empty()));
    }

    #[test]
    fn los_adaptadores_se_reconocen_por_prefijo() {
        for name in [
            "syon-storage-toml",
            "syon-platform-twitch",
            "syon-tts-piper",
            "syon-music-spotify",
            "syon-webhook-http",
        ] {
            assert_eq!(role_of(name), Some(Role::Adapter), "{name}");
        }
        assert_eq!(role_of("syon-storage"), None);
        assert_eq!(role_of("otra-cosa"), None);
    }

    #[test]
    fn lee_los_miembros_de_un_cargo_metadata() {
        let text = r#"{"packages":[
            {"name":"syon-domain","dependencies":[]},
            {"name":"syon-app","dependencies":[
                {"name":"syon-domain","kind":null,"path":"/x"},
                {"name":"serde","kind":"dev"}]}
        ],"workspace_members":[]}"#;
        let members = members_from_metadata(text).unwrap();
        assert_eq!(members.len(), 2);
        let app = members.iter().find(|m| m.name == "syon-app").unwrap();
        assert_eq!(app.role, Some(Role::App));
        assert_eq!(app.deps[0], dep("syon-domain"));
        assert_eq!(app.deps[1].kind, "dev");
        assert_eq!(app.deps[1].role, None);
        assert_eq!(
            check(&members),
            ["regla 2 (syon-app solo depende de syon-domain): syon-app → serde [dev]"]
        );
    }

    #[test]
    fn un_metadata_incompleto_da_error() {
        assert!(members_from_metadata("{}").is_err());
        assert!(members_from_metadata(r#"{"packages":[{"name":"x"}]}"#).is_err());
        assert!(members_from_metadata("no es json").is_err());
    }

    /// El workspace real cumple las reglas (y `cargo metadata` se puede leer con este lector).
    #[test]
    fn el_workspace_actual_cumple_las_reglas() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let members = load_members(&root).unwrap();
        assert!(members.iter().any(|m| m.name == "syon-domain"));
        assert_eq!(check(&members), Vec::<String>::new());
    }
}
