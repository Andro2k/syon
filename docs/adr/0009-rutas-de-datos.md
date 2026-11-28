# ADR-0009: Rutas de datos con `etcetera`, resueltas solo en `syon-engine`

- Estado: Reemplazada por ADR-0020
- Fecha: 2026-11-25

## Contexto

SYON guarda preferencias, un log y, más adelante, estado de ejecución y caché. Hay que decidir en qué
carpetas y quién las resuelve (diseño 001, sección 7). Los adaptadores no deben consultar el sistema
operativo, para que sus pruebas usen carpetas temporales. La crate habitual, `dirs`, arrastra
`option-ext`, con licencia MPL-2.0 (comprobado con `cargo deny`), y el [ADR-0004](0004-politica-de-dependencias.md)
exige un ADR explícito para cualquier licencia copyleft.

## Decisión

1. Se usa la crate `etcetera` 0.11 (MIT o Apache-2.0), con `choose_app_strategy` y el nombre de
   aplicación `SYON`, sin autor ni dominio. Su único paquete adicional en Linux es `cfg-if`; en Windows
   añade `windows-sys` y `windows-link` (MIT o Apache-2.0).
2. **Solo `syon-engine` consulta el sistema** (`AppPaths::from_system`). Los adaptadores reciben rutas por
   constructor; las pruebas usan `AppPaths::under(carpeta_temporal)`. Construir `AppPaths` no crea carpetas:
   las crea quien escribe (`TomlSettingsStore::save` crea la de configuración).
3. **Qué va en cada carpeta:**

   | Uso | Carpeta | Linux | Windows |
   |-----|---------|-------|---------|
   | `settings.toml` (preferencias) | configuración | `$XDG_CONFIG_HOME/syon` (por defecto `~/.config/syon`) | `%APPDATA%\SYON\config` |
   | `syon.log`, estado de ejecución | datos | `$XDG_DATA_HOME/syon` (por defecto `~/.local/share/syon`) | `%APPDATA%\SYON\data` |
   | Datos regenerables | caché | `$XDG_CACHE_HOME/syon` (por defecto `~/.cache/syon`) | `%LOCALAPPDATA%\SYON\cache` |

4. El log va en la carpeta de **datos** y no en la de estado (`state_dir`): `etcetera` la ofrece solo en
   Linux (en Windows devuelve `None`) y así ambas plataformas usan la misma regla.
5. Alcance: Windows y Linux. En macOS `etcetera` usa XDG para esta estrategia; si SYON llega a macOS se
   revisará con su propio ADR.

## Verificación de la correspondencia con Windows

- En el código de `etcetera` 0.11 (`app_strategy/windows.rs` y `base_strategy/windows.rs`), la estrategia de
  Windows toma la carpeta Roaming de `%APPDATA%` (o, si falta, de la API `SHGetKnownFolderPath` con
  `FOLDERID_RoamingAppData`) para configuración y datos, y `%LOCALAPPDATA%` (`FOLDERID_LocalAppData`)
  para la caché; añade `<autor>\<aplicación>` y el nombre `config`, `data` o `cache`. Con autor vacío
  queda `SYON\config`, `SYON\data` y `SYON\cache`.
- La prueba `en_windows_las_carpetas_corresponden_a_appdata_y_localappdata` compara el resultado con
  `%APPDATA%` y `%LOCALAPPDATA%`. Solo se compila en Windows, así que se ejecuta en el trabajo de Windows
  de la CI; no se pudo ejecutar en el entorno donde se escribió este parche (Linux).
- En Linux, `en_linux_las_carpetas_siguen_xdg` se ejecuta en cada comprobación.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| `dirs` | Arrastra `option-ext` (MPL-2.0) y exigiría un ADR de licencia copyleft |
| `directories` | Misma cadena de dependencias que `dirs` |
| Resolver las rutas a mano con variables de entorno | Duplica lo que `etcetera` ya hace, incluida la API de carpetas conocidas de Windows |
| Variable de entorno propia para la ubicación | Las rutas por constructor resuelven las pruebas sin una variable más |
| Carpeta junto al ejecutable (portátil) | Puede no ser escribible; `AppPaths::under` deja la puerta abierta |

## Consecuencias

- Cambiar de crate de rutas toca un solo archivo (`syon-engine/src/paths.rs`).
- Configuración y datos comparten la carpeta Roaming de Windows, que puede sincronizarse entre equipos
  en entornos con perfil móvil; si el log o el estado crecen, conviene moverlos a `%LOCALAPPDATA%` con un
  ADR nuevo.
- `etcetera` es pre-1.0: se actualiza con parche propio.
- Si falta la carpeta personal, `from_system` devuelve `PathsError::HomeDirUnavailable` y el binario
  decide cómo avisar.
