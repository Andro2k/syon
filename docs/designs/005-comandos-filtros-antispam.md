# Diseño 005: comandos, filtros y antispam (segunda parte de F3)

- Estado: Propuesta; redactado el 2026-11-27
- Fase: F3
- Fecha: 2026-11-27

## 1. Objetivo y alcance

Procesar el flujo unificado de chat para ejecutar comandos configurables, descartar mensajes que el
usuario quiera filtrar y limitar respuestas repetitivas. Este diseño completa F3 tras el [diseño
004](004-chat-unificado-y-envio.md): consume eventos `EventKind::Chat` y envía respuestas por el
puerto `ChatSender` ya implementado.

**Dentro:** roles mínimos del autor en `ChatMessage`; identificación de mensajes propios; parseo y
despacho de comandos, incluidos los que requieren rol de moderación; filtros de mensajes y usuarios;
antispam para evitar respuestas y comandos repetidos; configuración y pruebas.

**Fuera:** edición visual de comandos y reglas (F9); moderación automática que borre, banee o silencie
usuarios; respuestas, anuncios o mensajes privados de plataforma; acciones de moderador distintas de
responder al chat; persistencia remota o sincronización de configuración; ejecutar comandos escritos
por SYON u otro bot.

## 2. Punto de partida y decisiones heredadas

- `EventKind::Chat(ChatMessage)` llega al bus con plataforma, id, autor visible y texto. Los
  conectores de Kick y Twitch ya normalizan esos campos.
- `ChatSender` y `ChatHandle` envían texto a una o varias plataformas. El envío se ejecuta fuera del
  consumidor del bus; `EventSink::publish` nunca espera.
- F1 difirió roles e ids de usuario hasta que F3 los necesitara ([diseño 001](001-nucleo-dominio-eventos.md),
  sección 4). El diseño 004 dejó ambas decisiones a esta parte.
- Está confirmado en vivo que Kick y Twitch devuelven al feed los mensajes de la cuenta que envía.
  Por eso, todo mensaje propio se excluye de comandos, filtros con efectos y antispam antes de
  despachar reglas. No basta con asumir que un mensaje emitido por SYON no volverá como evento.

## 3. Roles y procedencia del autor

Ampliar `ChatMessage` con el identificador estable de plataforma cuando esté disponible y una
colección de roles normalizados. Los nombres visibles siguen en `User` para presentar mensajes, pero
no se usan para autorizar comandos. La propuesta de roles booleanos —ausentes significan `false`— es:

```rust
pub struct ChatMessage {
    pub id: MessageId,
    pub author: User,
    pub author_id: Option<String>,
    pub roles: ChatRoles,
    pub is_self: Option<bool>,
    pub text: String,
}

pub struct ChatRoles {
    pub broadcaster: bool,
    pub moderator: bool,
    pub vip: bool,
    pub subscriber: bool,
}
```

`is_self` significa que el emisor del mensaje es la cuenta autenticada que SYON usa para esa
plataforma: `Some(true)` es propio, `Some(false)` es otro autor y `None` es desconocido. Lo calcula el
adaptador al traducir el evento, comparando ids de plataforma; no se infiere del texto ni de un nombre
visible. `author_id` conserva la identidad para diagnóstico y futuras funciones, pero los comandos
consultan `is_self` y `roles`. El consumidor excluye tanto `Some(true)` como `None`: un falso positivo
de exclusión pierde una activación, mientras un falso negativo puede crear un bucle.

Para Twitch, `channel.chat.message` entrega `chatter_user_id` y banderas de broadcaster, moderator,
VIP y subscriber; las insignias están disponibles si en el futuro se requiere detalle adicional. El
adaptador debe mapear esas banderas y comparar `chatter_user_id` con la cuenta conectada. La
documentación oficial describe esos campos en [Channel Chat Message de EventSub](https://dev.twitch.tv/docs/eventsub/reference/chat/).

Para Kick, se extraerán id y roles solo de campos observados en eventos reales o definidos por la
documentación oficial. El esquema actual de `ChatMessageEvent` leído por el adaptador solo consume
`sender.username` y `content`; por tanto, el formato exacto de id, roles y la identidad de la cuenta
propia queda **por confirmar en vivo**. No se inventarán nombres de campos ni se derivarán permisos
de moderación de insignias desconocidas. Mientras no haya datos suficientes, Kick conserva roles
`false` y no habilita comandos que exijan esos roles; la detección propia usa el id si se confirma y,
si no, el mecanismo de correlación descrito abajo.

## 4. Exclusión de mensajes propios y prevención de bucles

La condición `is_self` se aplica en el consumidor antes de filtrar o despachar. Los mensajes propios
se siguen publicando en el bus y se pueden mostrar en la consola o interfaz; simplemente no producen
otra respuesta automática. Así se conserva el eco confirmado por ambas plataformas sin que una
respuesta `!hola` dispare `!hola` de nuevo.

La fuente preferida es la comparación del id de autor con la identidad de la cuenta autenticada. Si el
feed no entrega ese id, el adaptador mantiene una ventana acotada de ids de mensajes confirmados por
`ChatSender`: registra el id devuelto al enviar y marca como propio el eco con ese mismo id. Como el eco
puede llegar antes de la respuesta HTTP, el adaptador también puede correlacionar temporalmente por
plataforma, canal, texto exacto y una ventana corta; una coincidencia no confirmada nunca autoriza una
acción, solo excluye de automatización. La ventana tiene límite de tamaño y vencimiento para que un
fallo de eco no acumule estado. No se registran textos de chat en logs.

Si no hay identidad ni correlación fiable, `is_self` queda como `None` y el mensaje no se automatiza. Esta
regla conservadora debe quedar contada como diagnóstico sin texto, para revelar que una plataforma no
está pudiendo distinguir autores.

## 5. Procesamiento de mensajes

Un único consumidor de automatización conserva el orden de llegada del bus y aplica estas etapas:

1. Ignorar eventos que no sean `EventKind::Chat`.
2. Excluir mensajes propios y mensajes sin identidad suficiente para automatizarlos.
3. Aplicar filtros de entrada (plataforma, usuario, texto y duplicados).
4. Reconocer el prefijo de comando y tokenizar el nombre y argumentos.
5. Buscar comando habilitado, validar plataforma y rol mínimo, y comprobar el antispam.
6. Ejecutar la acción y enviar la respuesta mediante `ChatHandle`; un fallo de envío se registra sin
   volver a publicar la respuesta como una nueva acción.

El consumidor no realiza I/O de red dentro de `publish` ni bloquea el hilo del feed. Una carga lenta
no debe detener las plataformas: usar una cola acotada dedicada al consumidor y registrar los descartes
por saturación. El bus ya permite asignar capacidad por suscriptor según ADR-0007.

## 6. Comandos

Un comando es una entrada de configuración con nombre, alias opcionales, respuesta, estado habilitado,
plataformas permitidas y rol mínimo requerido. Se propone un prefijo configurable, `!` por defecto;
solo un prefijo al inicio del texto activa el parser. El nombre no distingue mayúsculas de minúsculas;
los argumentos conservan el texto restante y se separan por espacios Unicode, sin evaluación de
expresiones ni plantillas ejecutables.

El primer conjunto de acciones admite texto literal y variables acotadas (`{user}` y `{platform}`),
con sustitución escapada como texto plano. No hay llamadas a shell, scripts, expresiones, encadenado de
comandos ni una API de plugins en F3. Un nombre desconocido no responde. Una respuesta puede dirigirse
a la plataforma de origen o a la lista configurada; el valor inicial es la plataforma de origen para
no duplicar respuestas ni alcanzar canales no previstos.

El nivel mínimo se expresa como `Everyone`, `Subscriber`, `Vip`, `Moderator` o `Broadcaster`. La
jerarquía de autorización es acumulativa: broadcaster satisface todos los niveles; moderator satisface
Vip, Subscriber y Everyone; VIP satisface Subscriber y Everyone. El adaptador conserva flags
independientes y el motor de comandos aplica esta jerarquía. Si faltan datos de rol, solo se autorizan
comandos `Everyone`.

Los nombres duplicados o alias en conflicto invalidan la carga de configuración: se informa el error y
se conserva el último conjunto válido. Los comandos deshabilitados y los desconocidos no producen
respuesta automática.

## 7. Filtros y antispam

Los filtros de entrada son reglas configuradas, evaluadas antes del parser y en orden estable:

- plataformas incluidas o excluidas;
- usuarios ignorados, comparados por id estable cuando exista y por nombre normalizado como respaldo;
- frases o expresiones simples de inclusión/exclusión, sin regex en la primera versión;
- duplicados por id de mensaje y por huella de contenido, con ventana de retención acotada.

Una regla puede descartar un mensaje o impedir comandos. En F3 los filtros no borran mensajes,
sancionan usuarios ni responden explicando el rechazo. El chat original permanece visible en el bus.

El antispam opera localmente por plataforma y cuenta, con límites configurables de ejecuciones por
comando y un enfriamiento mínimo entre respuestas idénticas. Los valores iniciales propuestos son un
intervalo de 5 segundos por usuario y comando, y 1 respuesta por segundo global por plataforma; los
límites de Twitch y Kick siguen aplicándose en el servidor y sus rechazos no se reintentan. Un rechazo
por antispam es silencioso para no amplificar spam. Los contadores usan reloj monotónico, vencen solos
y tienen tamaño máximo; reiniciar SYON los reinicia. No se limita el mensaje escrito manualmente por
la persona operadora en esta parte.

## 8. Configuración y persistencia

La configuración pertenece a `syon-app` como modelo sin `serde`; `syon-storage-toml` define su
serialización. Mantener la dirección de dependencias: dominio → app → adaptadores, con `syon-engine`
ensamblando el consumidor, el almacenamiento y los senders. Los adaptadores no dependen del motor.

Los ajustes se guardan en una sección nueva de `settings.toml`, con versión de esquema existente. La
carga valida prefijo, nombres, destinos, roles y límites; datos inválidos producen un diagnóstico en
español y conservan el último estado válido. No se registran respuestas, contenido filtrado, nombres de
usuario ni ids de mensajes en logs ordinarios. Los secretos no forman parte de esta configuración.

No se propone dependencia directa nueva. En particular, las reglas de texto iniciales no necesitan un
motor de expresiones regulares.

## 9. Pruebas y criterios de salida

| Criterio | Evidencia |
|----------|-----------|
| Twitch traduce id y roles del evento real | Pruebas de traducción con banderas activadas y desactivadas |
| Kick mantiene roles desconocidos como falsos y no habilita permisos por inferencia | Pruebas del traductor y contraste en vivo pendiente |
| El eco propio se publica, pero no ejecuta comandos ni actualiza antispam | Prueba de consumidor con evento `is_self` |
| `is_self = None` no dispara automatización | Prueba de política conservadora |
| Prefijo, alias, argumentos, mayúsculas y comandos desconocidos siguen la regla | Pruebas del parser y despacho |
| Los roles se autorizan con la jerarquía definida | Tabla de pruebas por rol requerido y rol recibido |
| Filtros descartan antes del parser y respetan su configuración | Pruebas por plataforma, usuario, texto y duplicado |
| Un mensaje duplicado o una respuesta en enfriamiento no vuelve a producir respuesta | Pruebas de ventana y reloj controlado |
| Una falla del envío no bloquea el consumidor ni crea bucle | Sender falso con error y prueba de orden |
| Ningún log ni error contiene el texto del chat ni tokens | Pruebas de redacción de errores/logs |
| La cola llena cuenta descartes y la publicación nunca espera al consumidor | Prueba de saturación sin bloqueo |

## 10. Riesgos y pendientes

- El formato real de roles e identidad de Kick no está documentado en este repositorio ni se confirmó
  aún con un evento capturado; debe contrastarse antes de implementar la traducción.
- El emparejamiento de mensajes propios por id depende de que los senders devuelvan un id utilizable
  y que el feed publique el mismo id. La correlación temporal solo se usa para excluir, nunca para
  permitir comandos.
- La política de descartar al autor no identificable evita bucles a costa de perder comandos válidos
  en una plataforma mal configurada; hace falta exponer un diagnóstico útil en F9.
- Los límites locales son un cortafuegos conservador, no una garantía de cuotas de plataforma. Sus
  valores y unidad deben confirmarse con la experiencia en vivo y los rechazos oficiales.
- Persistencia y administración visual dependen de los diseños de ajustes y F9; esta parte solo define
  el modelo y su lectura inicial.

## 11. Plan de parches propuesto

Un parche por paso, con `docs/estado.md` actualizado en el mismo parche. El detalle de archivos y
pruebas se ajustará al aceptar este diseño.

1. `docs(design)`: este diseño y su entrada en el índice; actualizar la fila de F3 parte 2 a «Diseño
   propuesto, pendiente de aceptación».
2. `feat(domain)`: identidad, `ChatRoles` y `is_self` en `ChatMessage`, actualizaciones de adaptadores
   y pruebas de traducción; confirmar primero los campos reales de Kick.
3. `feat(app)`: modelos y puertos de comandos, reglas de filtro y antispam, sin dependencias de
   implementación.
4. `feat(storage)`: serialización TOML, validación, valores por defecto y pruebas de carga inválida.
5. `feat(engine)`: consumidor acotado, parser/despacho, filtros, deduplicación, antispam y envío de
   respuestas por `ChatHandle`, con pruebas aisladas.
6. `feat(syon)`: diagnóstico de configuración y cableado del ciclo de vida; sin añadir controles de UI.
7. `docs(walkthrough)`: walkthrough, tabla de paridad y estado al cerrar el paso.

## 12. Decisiones a aceptar

1. `ChatMessage` conserva id de autor opcional, roles normalizados y `is_self` triestado calculado por
   el adaptador; los mensajes inciertos se excluyen de automatización.
2. La identidad propia se resuelve por id de plataforma cuando exista y por correlación temporal
   únicamente como exclusión de seguridad.
3. Prefijo `!` configurable, alias, argumentos de texto y respuestas de texto literal con solo
   `{user}` y `{platform}`.
4. Niveles `Everyone`, `Subscriber`, `Vip`, `Moderator`, `Broadcaster` con jerarquía acumulativa.
5. Filtros de usuario, plataforma, texto y duplicados; sin acciones de moderación automáticas.
6. Límites locales iniciales de 5 s por usuario/comando y 1 respuesta/s por plataforma, silenciosos y
   sin reintento.
7. Consumidor asíncrono con cola acotada, descartes contados y envío fuera del camino de publicación.
8. Persistencia TOML en los ajustes existentes y sin nuevas dependencias directas.

La implementación no empieza hasta aceptar el diseño y confirmar los campos reales necesarios de
Kick; este documento constituye el entregable de documentación de este frente.
