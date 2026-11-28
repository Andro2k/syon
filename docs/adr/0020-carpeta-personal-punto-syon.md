# ADR-0020: Una sola carpeta `~/.SYON` para configuración, datos y caché

- Estado: Aceptada (2026-11-27; decisión del autor)
- Fecha: 2026-11-27

## Contexto

El [ADR-0009](0009-rutas-de-datos.md) repartió los archivos de SYON según las convenciones de cada sistema,
con la crate `etcetera`: XDG en Linux y `%APPDATA%` y `%LOCALAPPDATA%` en Windows. Al probar SYON en Windows,
el autor prefirió una única carpeta con nombre propio y la misma en ambos sistemas, como la de otras
herramientas de desarrollo (`.cargo`, `.claude`): una sola ubicación que encontrar, copiar o borrar, y
que no depende de qué carpeta del sistema toque a cada archivo.

## Decisión

1. **Todo cuelga de `~/.SYON`**, con `~` la carpeta personal del usuario, en Linux y en Windows:

   | Uso | Carpeta | Linux | Windows |
   |-----|---------|-------|---------|
   | `settings.toml` (preferencias) | configuración | `~/.SYON/config` | `%USERPROFILE%\.SYON\config` |
   | `syon.log`, `credentials.toml`, estado de ejecución | datos | `~/.SYON/data` | `%USERPROFILE%\.SYON\data` |
   | Datos regenerables | caché | `~/.SYON/cache` | `%USERPROFILE%\.SYON\cache` |

2. **La carpeta personal sale de `std::env::home_dir`** (`HOME` en Unix; `USERPROFILE` en Windows y, si falta,
   la API de carpetas conocidas). Con ello se quita la dependencia `etcetera`, que solo aportaba las
   carpetas por convención. Una carpeta personal ausente o relativa es `PathsError::HomeDirUnavailable`:
   una ruta relativa dejaría los archivos donde esté el directorio de trabajo.
3. **Se conserva del ADR-0009** lo que no cambia: solo `syon-engine` consulta el sistema
   (`AppPaths::from_system`); los adaptadores reciben rutas por constructor; las pruebas usan
   `AppPaths::under(carpeta_temporal)`; construir `AppPaths` no crea carpetas (las crea quien escribe) y el
   log y las credenciales viven en la carpeta de datos.
4. **No hay variable propia ni lectura de `XDG_*_HOME`.** Las pruebas del binario redirigen la carpeta
   personal con `HOME`.
5. **Sin migración automática.** Quien tenga archivos en las ubicaciones del ADR-0009 los mueve a mano
   (`%APPDATA%\SYON\config` a `.SYON\config`, `%APPDATA%\SYON\data` a `.SYON\data`; en Linux,
   `~/.config/syon` y `~/.local/share/syon`). Hoy solo hay una instalación en uso.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Mantener `etcetera` con XDG y `%APPDATA%` (ADR-0009) | Es lo que se prefirió cambiar: tres carpetas distintas por sistema y nombres distintos en Linux (`syon`) y Windows (`SYON`) |
| `~/.SYON` solo en Windows y XDG en Linux | El autor pidió la misma ubicación en ambos sistemas |
| Seguir con `etcetera` solo para la carpeta personal | Es un envoltorio de `std::env::home_dir`; una dependencia pre-1.0 sin aportar nada más |
| Una variable `SYON_HOME` para cambiar la raíz | Las rutas por constructor ya resuelven las pruebas; se añadirá si una instalación portátil lo pide |
| Caché fuera de `~/.SYON` (`%LOCALAPPDATA%`, `~/.cache`) | Rompe la idea de una sola carpeta; la caché es regenerable y hoy está vacía |

## Consecuencias

- Una sola carpeta que copiar, respaldar o borrar al desinstalar. En Windows deja de sincronizarse con
  perfiles móviles (Roaming), lo que también saca de ahí los tokens y el log.
- Quien personalizaba `XDG_CONFIG_HOME` y similares en Linux ya no lo ve reflejado.
- La caché entra en una copia de seguridad de `~/.SYON`; mientras esté vacía no importa, y si crece se
  reevaluará con un ADR.
- En Windows `.SYON` es una carpeta visible, no oculta: el punto es solo parte del nombre.
- El `AppPaths` de producción deja de tener una prueba distinta por sistema operativo; una sola comprueba
  `~/.SYON/{config,data,cache}`.
- Desaparece la dependencia `etcetera` y, con ella, su fila de `docs/deps.md`.
