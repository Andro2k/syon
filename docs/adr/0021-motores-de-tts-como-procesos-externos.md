# ADR-0021: Motores de TTS como procesos externos detrás de un puerto

- Estado: Propuesta
- Fecha: 2026-11-27

## Contexto

El [roadmap](../roadmap.md) deja para F4 «motor(es) elegidos mediante ADR y detrás de un trait», y la
[tabla de paridad](../requirements/paridad.md) pide varios motores (edge-tts, Piper y uno local). La regla de
dependencias de [AGENTS.md](../../AGENTS.md) dice que lo que dependa de Python o de un navegador corre como
proceso externo con versión fija, detrás de un trait, y que las licencias copyleft requieren un ADR explícito.

Los datos salen de búsquedas web del 2026-11-27 y no de la documentación oficial de cada proyecto; lo que
no se pudo contrastar se marca **por confirmar**.

- **Piper**: el proyecto original (`rhasspy/piper`, MIT) se archivó el 2025-10-06. El desarrollo sigue en
  `OHF-Voice/piper1-gpl`, con licencia GPL-3.0-or-later (versión 1.8.0 publicada en septiembre de 2026). El
  proyecto no concede licencia sobre los modelos de voz: cada voz hereda la de sus datos de entrenamiento.
- **edge-tts**: es un módulo de Python (`rany2/edge-tts`) que usa el servicio de lectura en voz alta de
  Microsoft Edge. No es una API de Microsoft con soporte ni contrato; hay informes de que sus términos solo
  la prevén para el lector de Edge, y en diciembre de 2025 Microsoft dejó de aceptar las llamadas desde
  navegadores, aunque el paquete de Python siguió funcionando. Puede dejar de funcionar sin aviso.
- **Motor local del sistema**: Windows trae voces SAPI y Linux suele tener `espeak-ng` (GPL-3.0). Ninguno
  necesita red ni descargas de modelos, y la calidad es menor.

## Decisión

1. **Un puerto `TtsEngine` en `syon-app`** que convierte texto en audio y no sabe nada del motor que hay
   detrás. La reproducción es otro puerto distinto (`AudioPlayer`): sintetizar y sonar se pueden probar y
   cambiar por separado.
2. **Cada motor es un proceso externo**, lanzado con `tokio::process` desde su propio adaptador, con la
   versión fija documentada en el diseño. SYON no enlaza ni incrusta ningún motor
   y no suma crates externos por ellos; solo activa la función `process` de `tokio`, que ya es dependencia
   (se anota en `docs/deps.md` en el parche que la active).
3. **Tres motores, uno por crate** (`syon-tts-piper`, `syon-tts-edge`, `syon-tts-system`), creados cuando
   se implementen y no antes ([ADR-0003](0003-arquitectura-hexagonal-workspace.md)):
   - **Piper** como motor local de calidad. Es GPL-3.0-or-later, así que solo se usa **como programa
     aparte que el usuario instala**: SYON no lo enlaza ni lo incluye en su repositorio, y se comunica por argumentos y archivos. Las voces las descarga el usuario; SYON no las
     redistribuye.
   - **edge-tts** como motor en la nube, **desactivado por defecto** y con un aviso al activarlo de que no
     es un servicio con soporte. Si falla, el engine pasa al siguiente motor de la lista de ajustes.
   - **Voz del sistema** como motor de respaldo sin red: SAPI en Windows y `espeak-ng` en Linux, ambos
     escribiendo un WAV.
4. **El audio es siempre un archivo en la caché** de `~/.SYON/cache` ([ADR-0020](0020-carpeta-personal-punto-syon.md)),
   que los motores escriben y el reproductor lee; se borra tras sonar. Así los tres motores comparten
   contrato, y el formato (WAV o MP3) lo declara el motor.
5. **La reproducción se decide en un ADR aparte** cuando llegue su parche, porque suma una dependencia de
   audio. El candidato es `rodio`, con decodificación de MP3 por el audio de edge-tts. Hasta entonces el
   puerto existe y las pruebas usan un reproductor falso.
6. **Sin motor, SYON sigue funcionando.** Un motor no instalado, con versión distinta de la esperada o que
   falla se registra con el prefijo `tts:` y no detiene el engine ni el bus.

## Alternativas consideradas

- **Enlazar Piper como biblioteca** (por ejemplo `piper-rs` o ONNX Runtime directo): arrastra la GPL al
  binario de SYON y obliga a revisar la licencia de todo el proyecto. Se descarta mientras SYON no
  decida ser GPL.
- **Un fork MIT de Piper o sherpa-onnx con modelos tipo Piper (Apache-2.0):** evita la GPL, pero añade un
  runtime de ONNX a las dependencias de Rust y un motor menos conocido. Queda como alternativa si el
  autor prefiere no depender de un programa GPL aparte.
- **Kokoro-82M (Apache-2.0)** como motor local en lugar del del sistema: mejor calidad que SAPI y sin
  copyleft, pero necesita un runtime que no está en el proyecto. Se puede añadir después como un adaptador más.
- **Hablar el protocolo de edge-tts desde Rust (`reqwest` y `tokio-tungstenite`):** se evita Python, pero
  SYON asumiría el mantenimiento de las cabeceras y los tokens de un servicio no oficial que ya cambió una
  vez. Un proceso externo deja ese riesgo en un paquete con mantenimiento activo.
- **Servicios de pago con API propia (Azure, ElevenLabs...):** quedan fuera de F4 por pedir claves y costes
  del usuario; encajarían como otro adaptador del mismo puerto.

## Consecuencias

- Se gana: la regla de AGENTS.md se cumple sin dependencias nuevas de Rust, un motor roto o retirado se
  cambia en un adaptador y el copyleft de Piper queda fuera del código de SYON.
- Se paga: el usuario instala Python y `edge-tts`, Piper y sus voces, o usa solo la voz del sistema; el
  arranque de un proceso por frase añade latencia (por medir en vivo).
- **Pendiente de contrastar en vivo:** los argumentos reales de la línea de comandos de Piper 1.x, de
  `edge-tts` y de `espeak-ng`; que SAPI escriba un WAV desde PowerShell sin dependencias; y los términos
  de edge-tts.
- **Pendiente antes de distribuir (F10):** si el instalador incluye Piper u `espeak-ng`, se cumplen sus
  obligaciones GPL (texto de la licencia y oferta del código fuente). Que esto no contagie a SYON por
  comunicarse solo por argumentos y archivos es la lectura habitual, pero conviene una revisión legal.
  Las licencias de cada voz también se revisan.
