# ADR-0021: Kokoro como único motor de TTS, ejecutado como proceso externo detrás de un puerto

- Estado: Propuesta; corregida el 2026-11-27 para dejar un solo motor
- Fecha: 2026-11-27

## Contexto

El [roadmap](../roadmap.md) deja para F4 «motor(es) elegidos mediante ADR y detrás de un trait». La
[tabla de paridad](../requirements/paridad.md) pedía varios motores (edge-tts, Piper y uno local). El autor
decidió el 2026-11-27 quedarse **solo con Kokoro** por ahora, con voces locales y claras. La regla de
dependencias de [AGENTS.md](../../AGENTS.md) dice que lo que dependa de Python o de un navegador corre como
proceso externo con versión fija, detrás de un trait, y que las licencias copyleft requieren un ADR explícito.

Los datos salen de búsquedas web del 2026-11-27 y no de una prueba de calidad ni de la documentación
completa de cada proyecto; lo que no se contrastó se marca **por confirmar**.

- **Kokoro-82M**: modelo de 82 millones de parámetros. Su código y sus pesos son Apache-2.0. Una
  comparativa del 2026-11-18 le da 8 idiomas y 54 voces; **que el español suene bien no está confirmado**.
- **sherpa-onnx** (Apache-2.0) ejecuta Kokoro con ONNX Runtime en CPU y trae el programa
  `sherpa-onnx-offline-tts`; proyectos externos ya lo lanzan como proceso con Kokoro y reproducen el WAV.
  También tiene un crate de Rust oficial, pero su script de compilación descarga binarios prefabricados si no
  se le indica una carpeta, lo que choca con `--offline` y con `cargo deny`. Los modelos de Kokoro con varios
  idiomas necesitan datos de `espeak-ng` (GPL-3.0) para pasar el texto a fonemas; **su licencia exacta dentro
  del paquete del modelo está por confirmar**.
- **`any-tts`** (MIT o Apache-2.0) ejecuta Kokoro dentro del proceso con Candle, sin ONNX. Está en la versión
  0.1.x y su README dice que el ajuste de paridad de Kokoro sigue en curso.

## Decisión

1. **Kokoro-82M es el único motor de TTS por ahora.** Piper, edge-tts y la voz del sistema quedan fuera: no
   hay lista de motores, orden de respaldo ni salto de motores caídos. Si se añade otro, será con otro ADR.
2. **Un puerto `TtsEngine` en `syon-app`**, con Kokoro como única implementación. El roadmap pide el trait y
   sirve para probar el engine con un motor falso; no se construye ninguna abstracción de selección encima
   (regla de abstracción justificada de AGENTS.md). La reproducción es otro puerto, `AudioPlayer`.
3. **Kokoro corre a través de `sherpa-onnx-offline-tts` como proceso externo**, lanzado con
   `tokio::process` desde un único adaptador, `syon-tts-kokoro`, con la versión del programa fija y
   documentada en el diseño. SYON no enlaza ni incrusta sherpa-onnx ni el modelo, y no suma crates externos;
   solo activa la función `process` de `tokio`, que ya es dependencia (se anota en `docs/deps.md` en el
   parche que la active).
4. **El usuario instala el programa y descarga el modelo**; SYON no los distribuye en F4. El audio es un
   archivo WAV en `~/.SYON/cache` ([ADR-0020](0020-carpeta-personal-punto-syon.md)) que el motor escribe, el
   reproductor lee y se borra después.
5. **La reproducción se decide en un ADR aparte** cuando llegue su parche, porque suma una dependencia de
   audio. El candidato es `rodio`.
6. **Sin motor, SYON sigue funcionando.** Un programa o un modelo que faltan, o un fallo al sintetizar, se
   registra con el prefijo `tts:` y no detiene el engine ni el bus.

## Alternativas consideradas

- **`any-tts` dentro del proceso:** evita lanzar un proceso por frase y el programa externo, pero es un crate
  joven con un árbol grande de dependencias (Candle) y sin medir en CPU. Queda como segundo adaptador del
  mismo puerto si la latencia del proceso resulta molesta; no hace falta cambiar nada más para añadirlo.
- **El crate oficial de sherpa-onnx dentro del proceso:** descarta el proceso externo, pero su build descarga
  binarios y enlaza una biblioteca nativa; se descarta por lo explicado arriba.
- **Piper (GPL-3.0-or-later):** voces claras y ligeras, pero copyleft y con licencia distinta por voz. Se
  deja fuera por ahora; reaparecería con su propio ADR.
- **edge-tts:** servicio no oficial de Microsoft que ya cambió sus requisitos una vez, y necesita red. Fuera.
- **Voces del sistema (por ejemplo el crate `tts`, MIT):** no necesitan modelos, pero la calidad es la del
  sistema y algunos backends hablan directamente sin devolver audio. Fuera por la calidad pedida.

## Consecuencias

- Se gana: un solo camino que probar y documentar, voces neuronales locales sin red y sin copyleft en el
  código de SYON, y un puerto que deja abierta la puerta a otros motores.
- Se paga: instalar sherpa-onnx y bajar un modelo de cientos de MB (el paquete inglés de Kokoro que usan
  otros proyectos ronda los 325 MB); lanzar un proceso por frase añade latencia (por medir en vivo); el
  español no está contrastado.
- **Pendiente de contrastar en vivo:** los argumentos reales de `sherpa-onnx-offline-tts` para Kokoro, qué
  paquete de modelo cubre español y su calidad, la latencia por frase en CPU y cómo recibe el texto.
- **Pendiente antes de distribuir (F10):** si el instalador incluye sherpa-onnx o el modelo, revisar la
  licencia de los datos de `espeak-ng` que lo acompañen y cumplirla.
