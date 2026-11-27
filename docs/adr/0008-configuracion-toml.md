# ADR-0008: Configuración en TOML, preferencias separadas del estado y formato propio del adaptador

- Estado: Aceptada
- Fecha: 2026-11-25

## Contexto

SYON necesita guardar las preferencias del usuario (en F1, el nivel de log) en un archivo que una
persona pueda leer y editar a mano. Cada fase posterior añade secciones. El puerto `SettingsStore` y
el modelo plano `Settings` ya viven en `syon-app`, que por el [ADR-0003](0003-arquitectura-hexagonal-workspace.md)
no depende de nada externo. Hay que fijar el formato, qué se guarda y qué no, y cómo se escribe sin
corromper el archivo (diseño 001, secciones 5 y 7).

## Decisión

1. **Formato TOML** en un archivo `settings.toml` con `schema_version`. Se usan las crates `toml`
   (con `std`, `serde`, `parse` y `display`) y `serde` con `derive`; **solo** `syon-storage-toml` las
   usa. `syon-app` sigue sin `serde` ni dependencias externas.
2. **Preferencias y estado de ejecución van en almacenes distintos.** `settings.toml` solo contiene lo
   que el usuario cambia a propósito. Lo que la app cambia sola (posición de la ventana, contadores) irá
   a otro almacén que se crea con su primer caso de uso. Los secretos no van en ninguno de los dos.
3. **Formato propio del adaptador.** `syon-storage-toml` tiene tipos privados para el archivo y traduce
   desde y hacia `syon_app::Settings`:
   - Al **guardar** se serializa un tipo con `#[derive(Serialize)]`.
   - Al **cargar** se lee un `toml::Table` y cada clave conocida se convierte por separado, de modo que
     un tipo incorrecto produce un error que nombra la clave completa (`logging.level: …`). Los errores
     de `toml` no incluyen la clave por sí solos, y el contrato exige ruta y clave.
4. **Comportamiento:**
   - Archivo ausente: `Settings::default()`; no se escribe nada hasta el primer `save`.
   - Clave ausente: valor por defecto. Clave desconocida: se ignora, para que un archivo escrito por una
     versión más nueva no rompa a una anterior.
   - Archivo ilegible (TOML roto, no UTF-8 o tipo incorrecto): `SettingsError::Invalid` con la ruta y,
     cuando se conoce, la clave. `load` nunca escribe, así que el archivo no se sobrescribe; quien llama
     no debe guardar los valores por defecto tras un `Invalid` (el engine aparta el archivo, ADR-0010).
   - Fallo de acceso al disco: `SettingsError::Io` con la ruta.
   - `save` crea la carpeta si falta, escribe un temporal en la misma carpeta (`settings.toml.<pid>.tmp`),
     lo sincroniza a disco y lo renombra sobre el destino. Si algo falla, borra el temporal y el archivo
     anterior queda intacto.
   - **Los guardados del proceso se serializan** con un cierre único: `SettingsStore` exige `Send + Sync`,
     y sin él dos `save` simultáneos (de hilos o de instancias distintas sobre el mismo archivo) comparten
     el nombre del temporal y uno falla con `NotFound` (reproducido con 8 hilos: fallaron 350 de 400
     guardados). Gana el último que entra. Se descartó un temporal distinto por llamada porque no evita
     dos `rename` simultáneos sobre el mismo destino, que según la plataforma pueden fallar (no se ha
     verificado en Windows; lo ejercita la CI).
5. **Sin `thiserror`:** los errores son los enums `Io` e `Invalid` de `syon-app`, con `Display` escrito a mano.
6. **La ruta se recibe por constructor** (`TomlSettingsStore::new`). Dónde vive el archivo lo decide
   `syon-engine` (ADR 0009); el adaptador no consulta el sistema operativo.
7. **Sin migraciones** hasta que exista una segunda versión de esquema. `schema_version` se conserva tal
   como se leyó.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| `#[derive(Deserialize)]` sobre todo el archivo | Menos código, pero el error no nombra la clave y el contrato lo exige; con una o dos secciones la lectura manual cuesta poco |
| `serde_path_to_error` | Añade una dependencia solo para el nombre de la clave |
| `toml_edit` (conservar comentarios y orden) | Útil si la UI reescribe el archivo editado a mano; se reevaluará en F9 y no hace falta para F1 |
| JSON o YAML | Peor para editar a mano; TOML ya es el formato de MiniKick |
| `serde` dentro de `syon-app` | Acopla el núcleo a un formato y rompe la regla de dependencias |
| Escribir directamente sobre el archivo | Un corte a mitad de escritura deja un TOML truncado |

## Consecuencias

- Cada sección nueva exige tipos de formato y lectura por clave en el adaptador: unas pocas líneas por
  clave, el precio de que `syon-app` no conozca ningún formato.
- Una versión anterior de SYON que guarde un archivo de una versión más nueva perderá las claves que no
  conoce. Se acepta mientras solo haya una versión de esquema; la migración, si llega, tendrá su ADR.
  El [ADR-0019](0019-guardado-con-esquema-mas-nuevo.md) cambia esto: `save` rechaza ese guardado.
- `save` reescribe el archivo entero y no conserva comentarios ni el orden que el usuario haya puesto.
- El `rename` atómico sobre un archivo existente depende de la plataforma: la biblioteca estándar lo
  reemplaza en Linux y en Windows. Se prueba en ambos con la CI.
- Las pruebas fijan el contrato con carpetas temporales: archivo ausente, ida y vuelta, claves
  desconocidas, contenido inválido sin sobrescribir y fallo de guardado con el original intacto.
