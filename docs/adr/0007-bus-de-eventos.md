# ADR-0007: Bus de eventos con un canal acotado por consumidor

- Estado: Aceptada
- Fecha: 2026-11-25

## Contexto

Las plataformas publican eventos y varios módulos (comandos, TTS, overlays, webhooks) los
consumen a distinto ritmo. El chat es el centro del sistema: un consumidor lento no debe afectar
a los demás ni a quien publica (diseño 001, sección 6).

## Decisión

`EventBus`, en `syon-engine`, implementa `EventSink` con estas reglas:

- Cada consumidor se suscribe con un nombre y una capacidad y recibe su propio
  `tokio::sync::mpsc` de `Arc<Event>`; el evento no se copia por consumidor.
- `publish` es síncrono y usa `try_send`: nunca espera.
- Si la cola de un consumidor está llena, el evento se descarta **solo para ese consumidor**, se
  incrementa su contador y se avisa en el log como máximo una vez por segundo.
- `stats()` devuelve por consumidor nombre, capacidad, eventos en cola y descartados.
- Un consumidor que suelta su receptor se retira del bus.
- Un consumidor que no puede perder eventos (comandos, recompensas) se suscribe con más capacidad
  y su contador de descartes se trata como un fallo visible.
- Solo hay una política de desbordamiento (descartar el evento nuevo); otra, como descartar el más
  antiguo, se añade cuando un consumidor concreto la necesite.
- `tokio` se usa solo con la función `sync`: el bus no necesita runtime para publicar.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| `tokio::sync::broadcast` | Más simple, pero un consumidor lento pierde eventos de forma silenciosa para el código (solo se entera con `Lagged`) y una única capacidad sirve a todos por igual |
| Canal con bloqueo (`send().await`) | Un consumidor lento frena a Kick y a Twitch |
| Pipeline secuencial de procesadores | Acopla los módulos entre sí |
| `flume` o `async-channel` | Sin ventaja clara sobre `tokio`, que los adaptadores de red usarán de todos modos |

## Consecuencias

- Bajo saturación se pierde información; la mitigación es el contador, el aviso y la capacidad por
  consumidor, no evitar el descarte.
- Los productores solo conocen `EventSink`; el bus es sustituible sin tocarlos.
- Las pruebas fijan el comportamiento: entrega a varios consumidores sin copiar, descarte contado
  de un consumidor detenido sin afectar a otro ni bloquear, retirada al soltar el receptor y
  publicación concurrente.
