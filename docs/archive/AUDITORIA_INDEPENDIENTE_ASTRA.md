# Auditoría independiente de segunda instancia — ImpactSampler Studio

Fecha: 2026-11-07. Revisión estática del árbol disponible. Informe solicitado: `docs/AUDITORIA_INDEPENDIENTE_ASTRA.md`.

## Alcance, método y límites

Se leyeron primero `README.md` e `ImpactSampler Studio - SPEC.md`. La fase independiente examinó las rutas de producción de `src-tauri/` y `src/`, con atención al callback, IPC, MIDI, DSP, reconstrucción de streams y efectos React. Se cerró un inventario independiente antes de abrir `docs/archive/INFORME_AUDITORIA_CODEX.md`. Después se contrastó ese informe con nuevas lecturas dirigidas. No se examinó `web-app`, no se implementaron correcciones ni se ejecutaron operaciones destructivas.

No se ejecutaron builds, tests Rust/React, la aplicación, hardware ASIO ni un perfilador: esas operaciones no son necesarias para la demostración estática y algunas escribirían artefactos. Se ejecutaron búsquedas, lecturas numeradas y un modelo aritmético en memoria del reloj ARP; este modelo **no es una ejecución del motor Rust**. No se auditó el interior de dependencias ni se certifican sus garantías. Las pruebas de intervención de este documento están propuestas, no superadas.

Huella de referencia: SHA-256 agregado de los 35 archivos de `src/` y `src-tauri/src/`, ordenados por ruta, concatenando ruta UTF-8, NUL y contenido: `e7639f5e99db31ad7186dd31bd90b9d17252c3888474f055b7a0b9c902a4917b`. Las referencias siguientes son relativas al repositorio y los rangos son inclusivos.

**Clasificación:** CONFIRMADO identifica evidencia suficiente; PARCIALMENTE CONFIRMADO identifica un mecanismo con condición todavía pendiente; NO DEMOSTRADO no sostiene la conclusión; FALSO POSITIVO significa contradicción con el código. HALLAZGO NUEVO identifica una omisión del informe anterior, y se acompaña del estado de evidencia. La confianza evalúa el razonamiento sobre el código, no la frecuencia del incidente.

**Severidad:** alta = notas, patrones, configuración o continuidad de audio afectados de forma relevante; media = función concreta incorrecta o riesgo condicionado; baja = defecto menor o mantenimiento. Ninguna cifra de xruns, latencia observada o FPS se deduce de una búsqueda textual.

## Resultado principal

La afirmación de salida sample-accurate no se cumple en la integración actual: los relojes avanzan por muestra, pero sus eventos se aplican antes de renderizar el buffer. Tampoco se cumple la garantía de callback sin bloqueos ni asignaciones. Esto no demuestra que cada sesión produzca cortes, pero sí invalida las garantías estrictas declaradas.

La primera auditoría acierta en esas cuestiones y en varias rutas de pérdida y cleanup. Es incompleta en reconstrucción de estado, semántica de edición, MIDI Learn y ownership de notas. Su candidato a eliminación `PadSampler::update_sample_rate` es un falso positivo. Las propuestas de división por dominios son opcionales; su tamaño no acredita una severidad funcional ni justifica una intervención previa a los bugs.

## Fase 1: evidencias independientes y comprobaciones posteriores

### A. Bugs funcionales demostrables

**A01 — Se pierde el offset de secuenciador, arpegiador y clic. CONFIRMADO; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/engine.rs`, `AudioRenderState::render_internal`, 1161–1184, 1193–1246 y 1321–1328. `src-tauri/src/audio/sampler.rs`, `trigger/render_mix`, 233–241 y 319–340. `src-tauri/src/audio/sequencer.rs`, `advance_sample/trigger_click_if_beat`, 716–784 y 579–590.

Ruta: un evento en el frame k>0 del buffer modifica el sintetizador o reinicia la posición del pad antes de renderizar los frames 0..N. El pad comienza en 0; el metrónomo reinicia su envolvente antes de producir el primer frame; el sintetizador recibe los eventos sin el tiempo transcurrido entre ellos. Para PCM el adelanto es k muestras, hasta 255/48000 = 5,3125 ms con N=256. No es una medición de latencia total. Dos triggers del mismo pad en un bloque reinician la misma voz dos veces sin audio intermedio; se pierde el primero. La granularidad interna de rustysynth podría añadir otras limitaciones: segmentar es necesario, pero debe verificarse la salida SF2 real antes de certificar precisión.

**A02 — Overflow silencioso y éxito IPC sin aplicación. CONFIRMADO; alta; confianza alta.**

Evidencia: `src-tauri/src/main.rs`, `main`, 1159–1170, `trigger_virtual_note`, 275–297, `load_soundfont`, 72–81; `src-tauri/src/midi/mod.rs`, callback de `connect_to_port`, 197–257; `src-tauri/src/audio/engine.rs`, `process_commands/send_ui_event`, 478–501 y creación de cola UI, 1805.

La cola de comandos real tiene **1024** entradas y la UI 128; no 512 como la especificación. Todos esos `push` ignoran el error. Ruta plausible: suspender audio desde ajustes, mantener la entrada MIDI activa, llenar la cola y soltar una nota cuyo NoteOn quedó encolado. Al reanudar se ejecuta el NoteOn sin su NoteOff. También puede anunciarse una carga o edición exitosa sin aplicarla. La pérdida de telemetría es distinta: un Step posterior puede recuperar posición, pero no necesariamente recupera un PatternUpdated o StateChanged descartado.

**A03 — Dos escritores ejecutan operaciones relativas sobre el espejo. CONFIRMADO; alta; confianza alta.**

Evidencia: `src-tauri/src/main.rs`, `sequencer_adjust_bpm`, 782–793; `sequencer_toggle_step`, 864–884; `sequencer_toggle_click/bank`, 920–952; `sequencer_undo`, 1087–1097. `src-tauri/src/audio/engine.rs`, `sync_shared_sequencer`, 486–489 y handlers 841–844, 866–890, 1071–1075.

Intercalación sin overflow: IPC encola ToggleBank; audio cambia A→B y copia B al espejo; IPC obtiene después el espejo y lo cambia B→A. El motor queda en B y la respuesta declara A. Un delta BPM puede aplicarse dos veces en el espejo, y Undo consumir dos snapshots en él. La UI puede recibir el evento correcto antes de una respuesta IPC incorrecta que lo sobrescribe. El mutex evita data races de memoria, **no** esta carrera lógica. Además, fallar un `try_lock` de publicación no deja una actualización pendiente. Es un defecto más fuerte que la mera divergencia condicionada a cola llena mencionada originalmente.

**A04 — Desconectar o cambiar puerto no termina las notas. CONFIRMADO; alta; confianza alta.**

Evidencia: `src-tauri/src/midi/mod.rs`, `disconnect`, 93–97; `connect_to_port`, 128–136; `start_watchdog`, 362–376. `src/hooks/useMidiStream.ts`, listener de estado, 120–127.

Con NoteOn activo, el cable se retira antes del NoteOff o se cambia el puerto. Solo se descarta la conexión y se publica estado; no se libera la voz, el pedal, el estado ARP ni los mapas visuales. Una voz sostenida o el fallback pueden continuar; no toda muestra percusiva quedará sonando indefinidamente. La limpieza QWERTY por blur no cubre las notas físicas.

**A05 — Cambiar sample rate cambia velocidad y afinación del PCM cargado. HALLAZGO NUEVO, confirmado; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/sampler.rs`, `from_wav_reader`, 109–143, `update_sample_rate`, 225–230 y `render_mix`, 319–340; `src-tauri/src/audio/engine.rs`, `AudioRenderState::new`, 368–376; construcción CPAL 1504–1508 y ASIO 1626–1629.

Se remuestrea una vez al cargar. El nuevo render reutiliza los mismos Arc; actualizar el sampler solo cambia el fade, y el cursor avanza un frame PCM por frame DAC. Cargar a 48 kHz y pasar a 44,1 kHz alarga duración por 48000/44100 = 1,088435 y reduce afinación. `reload_factory_assets` se ejecuta en setup (main.rs:1274–1278), no en cada cambio de dispositivo. Esto también puede afectar muestras de fábrica después del arranque. La corrección debe preservar muestras de usuario, sin sustituirlas por el kit de fábrica.

**A06 — Reconstruir el stream pierde configuración y restaura un transporte obsoleto. HALLAZGO NUEVO, confirmado; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/engine.rs`, `AudioRenderState::new`, 418–420 y 468–473; restauración SF2, 1510–1517 y 1631–1637; `process_commands`, 507–530; `render_internal`, 1193–1225. `src/components/AudioSettingsModal.tsx`, apertura/cancelación, 237–242 y 623–627; `src/components/PerformanceBay.tsx`, 39–43.

Abrir ajustes y cancelar destruye/recrea el stream. El nuevo ARP usa valores de fábrica en vez de `shared_arpeggiator`; los CC aprendidos vuelven a los predeterminados y el mute a false. La interfaz no vuelve a enviar los mapeos ni a leer el ARP en cada reinicio. Del preset SF2 se conserva patch, pero no banco: un preset de banco no cero se restaura como el patch del banco por defecto.

El secuenciador sí se clona, pero su espejo solo se actualiza en comandos; avanzar audio no lo publica. Play recién sincronizado deja `need_start_trigger=true` en el espejo, aunque audio ya lleve compases. Reiniciar puede volver a tick 0; con otra última publicación puede volver a una posición antigua. Conservar ajustes no implica resucitar notas sostenidas ni colas antiguas: esas políticas deben ser explícitas.

**A07 — Cambios de ARP pueden dejar voces sin NoteOff. HALLAZGO NUEVO, confirmado; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/engine.rs`, `process_commands`, 532–575 y 1080–1086; `src-tauri/src/audio/arpeggiator.rs`, `set_enabled`, 428–440, `set_rate`, 404–406 y `advance_sample`, 338–343, 359–381.

Ruta 1: tocar una nota con ARP apagado, activarlo sin soltarla, luego soltar. NoteOff se entrega al ARP, que no registró ese NoteOn; no llega a la voz directa.

Ruta 2: sonar una nota ARP larga, cambiar a una subdivisión rápida antes de su gate, y alcanzar el nuevo step antes del vencimiento anterior. El trigger reemplaza `active_playing_note` sin liberar la anterior. A 48 kHz/120 BPM, una negra tiene gate de 19200 muestras; pasar inmediatamente a 1/32 permite otra nota tras 3000 muestras. Con dos alturas distintas queda una voz sin release. Cambiar BPM bruscamente o retrigger de HOLD merece el mismo test. No basta enviar un único NoteOff al apagar ARP si ya se perdió la identidad de la voz anterior.

**A08 — El release del ARP detiene su reloj una muestra. HALLAZGO NUEVO, confirmado; media; confianza alta.**

Evidencia: `src-tauri/src/audio/arpeggiator.rs`, `advance_sample`, 338–356. El retorno de NoteRelease ocurre antes de sumar al reloj. Con acorde sostenido, 48 kHz, 120 BPM y 1/16, la primera nota llega en la llamada 6000 y las siguientes en 12001, 18002, 24003. El modelo aritmético en memoria reprodujo intervalos 6001; falta ejecutar el test Rust equivalente. Se añade una muestra por gate, además del error de integración A01. El test existente 463–481 solo observa el primer trigger, por lo que no detecta la deriva.

**A09 — Undo no identifica banco y no cubre todas las mutaciones. HALLAZGO NUEVO, confirmado; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/sequencer.rs`, `undo_history`, 274; `toggle_bank`, 397–410; `save_undo_state/undo`, 489–505; `clear_track/clear_pattern`, 419–433 y `requantize_pattern`, 544–548. Handlers de audio en engine.rs:871–874, 897–905 y 1061–1064.

Editar A guarda solo BankPattern. Cambiar a B y pulsar Undo escribe el snapshot de A sobre B, sin indicar ni restaurar el banco de origen. B puede contener trabajo previo. Clear y requantize tampoco guardan su estado anterior: Undo después de ellos puede deshacer otra operación o no hacer nada. Corregir A03 no resuelve este defecto interno.

**A10 — La cuadrícula muestra eventos que no puede borrar y oculta los del cierre de compás. HALLAZGO NUEVO, confirmado; media; confianza alta.**

Evidencia: `src-tauri/src/audio/sequencer.rs`, `get_active_pattern`, 335–350; `set_step/toggle_step`, 435–482; `record_live_hit`, 510–540. `src/components/StepSequencer.tsx`, `applyStepStroke`, 397–415; `src/App.tsx`, `handleSequencerSetStep`, 694–716.

Grabar sin cuantización en tick 23 ilumina el paso de tick nominal 24; borrar esa celda intenta eliminar únicamente tick 24, y puede retornar antes al considerar que ya está inactiva. El evento 23 sigue sonando mientras el estado optimista de React lo oculta. Tick 383 proyecta paso 16 en grilla de 16 pasos y se descarta, sin módulo circular. Hace falta una regla única para proyección y edición, que conserve el microtiming.

**A11 — MIDI Learn de pads cambia la nota de salida, no el enrutamiento de entrada; controles virtuales pueden mover otro control. HALLAZGO NUEVO, confirmado; alta; confianza alta.**

Evidencia: `src/hooks/useMidiStream.ts`, `syncMappingsToRust`, 67–75 y visualización 272–287; `src-tauri/src/main.rs`, `set_midi_learn_mappings`, 613–627; `src-tauri/src/midi/parser.rs`, `get_pad_index`, 143–154; callback MIDI en midi/mod.rs:197–218.

Aprender nota 60 para pad 1 hace que la UI ilumine pad 1. Rust sigue clasificando pads solo en canal 10, notas 36..43; nota 60 llega como nota melódica. El cambio además establece la nota SF2 de salida del pad a 60. Un mapa de entrada no debería sustituir una selección de instrumento de batería.

Segundo mecanismo: `triggerVirtualFader/Knob` (useMidiStream.ts:466–489) emite CC fijos 38+i/56+i. El motor busca primero el mapa aprendido (engine.rs:592–618, 621–649). Si K2 aprende CC56, mover K1 virtual manda CC56 y modifica K2. Usar comandos virtuales por índice evita esa reinterpretación sin cambiar la compatibilidad MIDI física.

**A12 — Salir con el ratón del teclado libera notas físicas ajenas al gesto. HALLAZGO NUEVO, confirmado; alta; confianza alta.**

Evidencia: `src/components/HardwareSkeleton.tsx`, construcción de keys, 206–225, y `onMouseLeave` del riel, 897–904; `src/App.tsx`, props, 933–950; `src/hooks/useMidiStream.ts`, 266–269 y `triggerVirtualNote`, 431–445; backend main.rs:275–282.

Mantener C4 en el controlador, pasar el ratón por el teclado y salir sin haber pulsado con él: `k.isActive` viene del mapa global, así que se envía NoteOff a C4. Un efecto análogo aparece en pads Gate: StepSequencer.tsx:1060–1063 usa la actividad global para decidir el release. Registrar las notas originadas por cada gesto resuelve esta interferencia local; no requiere rediseñar toda la polifonía.

**A13 — Gestos cancelados dejan listeners y pitch bend activo. CONFIRMADO; media; confianza alta.**

Evidencia: `src/components/HardwareSkeleton.tsx`, `handleFaderPointerDown/handleKnobPointerDown/handlePitchWheelPointerDown/handleModWheelPointerDown`, 234–335.

Ya se usa `setPointerCapture`; la omisión es la finalización. Solo pointerup quita listeners globales y centra pitch. pointercancel sin pointerup deja el gesto activo y futuros movimientos siguen mandando IPC. No hay limpieza por blur, lostpointercapture o desmontaje de esos listeners. No atribuir este mismo defecto al drag de grilla, que sí tiene pointercancel/blur y cleanup en StepSequencer.tsx:380–395.

**A14 — Respuestas de configuración pueden sobrescribir una selección posterior. CONFIRMADO; media; confianza alta en mecanismo, media en orden efectivo del backend.**

Evidencia: `src/components/AudioSettingsModal.tsx`, `probeDevice`, 191–226; efecto de apertura 237–306; `handleHostChange/handleDeviceChange`, 330–374.

Se selecciona A y luego B con A todavía pendiente. Si la respuesta A llega al final, se asignan disponibilidad, canales, frecuencias y sampleRate de A a la selección B. También puede continuar una inicialización antigua tras cerrar/reabrir. No hay número de petición ni validación de selección. La reproducción con promesas controladas demuestra el defecto del frontend; no se afirma que toda plataforma reordene todas las llamadas síncronas de Tauri. Serializar solo el dispositivo nativo tampoco vuelve correctos todos los resultados antiguos del cliente.

**A15 — Compensación de latencia depende erróneamente del tempo y usa otra ruta de medición. HALLAZGO NUEVO para dependencia del BPM; confirmado; media; confianza alta.**

Evidencia: `src/components/AudioSettingsModal.tsx`, efecto de calibración, 412–418, 438–456, 488–525; guardado 1175–1182. `src-tauri/src/audio/sequencer.rs`, `record_live_hit`, 514–523 y `set_bpm`, 371–383.

Se convierten ms a ticks a 120 BPM y se guardan/aplican los mismos ticks a cualquier BPM. Un resultado de 4 ticks equivale a 20,833 ms a 120, pero a 41,667 ms a 60. Si el objetivo declarado es compensar USB/buffer, esa duración física no debería duplicarse al bajar tempo. El clic sale por un AudioContext sin selección del endpoint del motor; el golpe se fecha al llegar a JS, no al capturarse en MIDI. Mezcla reacción humana, IPC y otra salida. El botón de calibración **reanuda el motor** (975–981): sería incorrecto afirmar que siempre permanece suspendido durante la medición. La discrepancia de dispositivos es condicional; la mezcla de relojes y la conversión fija son visibles.

**A16 — El watchdog compite con cambios manuales y no arranca si falla el audio inicial. HALLAZGO NUEVO, confirmado; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/engine.rs`, `apply_audio_config`, 2128–2140 y 2286–2319; `suspend_audio_engine`, 2381–2401; `spawn_watchdog`, 2864–2944; `start`, 2766–2819; main.rs:1162–1165.

Después de una invalidación, el watchdog puede estar construyendo un stream mientras IPC suspende o aplica otro: solo la asignación del holder está protegida, no la operación completa. Su resultado tardío puede reactivar audio tras Suspend o reemplazar el stream manual. Incluso una configuración manual exitosa no limpia `device_invalidated`, por lo que el watchdog puede destruirla después. El mutex exterior de AppState no protege el watchdog, que usa clones internos.

Si el arranque sin dispositivo retorna por `?`, no alcanza `spawn_watchdog`. main registra el error y sigue; enchufar luego un DAC no tiene recuperación automática. Son dos rutas independientes; necesitan pruebas distintas. En ASIO no hay aquí un protocolo de reconexión equivalente, como ya advertía el informe anterior.

**A17 — Un SoundFont de usuario homónimo carga el instrumento de fábrica. HALLAZGO NUEVO, confirmado; media; confianza alta.**

Evidencia: `src-tauri/src/soundfont/mod.rs`, `scan_soundfonts`, 264–329; `load_soundfont_from_file`, 333–345; `import_soundfont_file`, 460–471. `src/App.tsx`, `handleImportSoundfont/handleSelectSoundfont`, 378–406.

Ambas bibliotecas exponen filename; la carga prueba fábrica primero. Importar un SF2 diferente con el mismo basename de uno de fábrica y seleccionarlo por filename abre el de fábrica. Los IDs `user-...` no participan en la carga. Además, otra importación con mismo nombre de usuario sobrescribe el archivo previo; requiere una política explícita de reemplazo o nombre único, no cambios de DSP.

**A18 — Mantener Espacio alterna Play/Pause repetidamente. HALLAZGO NUEVO, confirmado; media; confianza alta.**

Evidencia: `src/App.tsx`, `handleKeyDown`, 334–342. No filtra `e.repeat`; cada repetición del sistema operativo encola TogglePlay. El hook QWERTY sí filtra repeat (useQwertyKeyboard.ts:143–145), pero es otro listener y no evita el del transporte.

**A19 — Antiflam solo conserva el último pad. HALLAZGO NUEVO, confirmado; media; confianza alta.**

Evidencia: `src-tauri/src/audio/sequencer.rs`, campos 262–263, `record_live_hit`, 528–539, `advance_sample`, 756–768; engine.rs:676–681.

Grabar dos pads en tick 20 con cuantización 1/16 dirige ambos a tick 24. El segundo sobrescribe `anti_flam_pad`. Al llegar al 24 se suprime solo ese pad y el primero se redispara pese a haber sonado en vivo. El caso está dentro de la ventana declarada de seis ticks; no depende de interpretar como bug un golpe fuera de esa ventana.

### A-condicional. Mecanismos relevantes con límites explícitos

**A20 — Cleanup tardío de listeners de App. PARCIALMENTE CONFIRMADO; media; confianza alta en mecanismo.**

Evidencia: `src/App.tsx`, efecto de inicialización, 273–331 y 346–356. Cleanup antes de resolver listen no llama al unlisten que llega después. Es reproducible con montaje/desmontaje y promesas diferidas. Sin embargo, `src/main.tsx:7–9` monta App una vez y **no usa StrictMode**; no se acreditó una ruta normal de navegación que lo remonte. No sostengo una fuga recurrente de producción ni prioridad alta incondicional. El hook MIDI y drag/drop de App sí contemplan resolución tardía.

**A21 — Formatos ASIO no soportados se escriben como i32. HALLAZGO NUEVO, parcialmente confirmado; alta si se alcanza; confianza alta en rama, media en hardware.**

Evidencia: `src-tauri/src/audio/engine.rs`, `start_asio_streaming`, 1584–1587 y callback 1665–1723. Tras cuatro formatos explícitos, `_` escribe i32 sin rechazar el formato recibido. Un driver con Float64 o formato/endian diferente no recibe su representación correcta. El código no demuestra que el driver disponible pueda producirlo; tampoco demuestra un overrun específico. Rechazar formatos no implementados antes de iniciar es la corrección mínima. No convertir esta observación en una acusación genérica de corrupción por todo uso de unsafe.

**A22 — El borrado de SF2 no limita filename al directorio de usuario. HALLAZGO NUEVO, confirmado en frontera IPC; alta potencial, explotación externa no demostrada; confianza alta.**

Evidencia: `src-tauri/src/main.rs`, `remove_user_soundfont`, 60–63 y registro 1199; `src-tauri/src/soundfont/mod.rs`, `delete_user_soundfont`, 504–509. Se concatena un argumento IPC arbitrario y se borra si existe, sin exigir basename ni confinamiento. Una llamada desde la webview con una ruta absoluta o `../...` puede apuntar fuera de la biblioteca. La UI habitual pasa nombres del catálogo; no se encontró ni se afirma una vía remota para inyectar esa llamada. No se ejecutó ningún borrado. La misma raíz del problema no convierte la carga arbitraria en vulnerabilidad: la aplicación sí ofrece importación de rutas escogidas por el usuario.

### B. Riesgos arquitectónicos de tiempo real

**B01 — Bloqueos alcanzables desde el callback. CONFIRMADO; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/engine.rs`, `render_f32/i16/u16`, 1333–1336, 1361–1364, 1391–1394; `process_commands`, 507–518, 577–581, 715–776; `get_pad_slots_info`, 1911–1931. CPAL llama render en 1528–1545 y ASIO en 1663.

Consultar slots toma samples, modes y gains mientras construye un Vec y clona Strings; al mismo tiempo LoadPadSample, ClearPadSample o SetPadGain en audio espera con lock. SetMasterGain y cargas SF2 tienen otras esperas posibles. No basta observar que la entrada a consumer usa try_lock. No se afirma un deadlock específico en esa ruta: se demuestra una espera sin plazo RT. Los try_lock de snapshots evitan esa espera, pero pueden perder publicación y no proporcionan ownership exclusivo del render.

**B02 — Construcción/destrucción de recursos y resize en audio. CONFIRMADO; alta; confianza alta en ruta local.**

Evidencia: `src-tauri/src/audio/engine.rs`, `process_commands`, 507–518 y 765–775; `render_internal`, 1148–1155 y 1238–1240; reservas 443–460. `src-tauri/src/audio/sampler.rs`, `load_sample/clear_sample`, 263–273; main.rs:488–497 y 518–524.

Los sintetizadores se construyen en el callback y al sustituirse se destruyen allí. No se cuantifica la asignación interna de rustysynth sin auditar la dependencia; el problema de buffers y PCM sí queda demostrado localmente: si control ya reemplazó/eliminó su Arc compartido, el slot del render conserva el último Arc del PCM anterior y lo libera en el callback al procesar el comando. Es memoria dinámica de tamaño del archivo. Un callback mayor de 4096 produce resize de Vec; ninguna cota validada lo impide.

No se encontró lectura de WAV desde render_mix: carga y remuestreo viven fuera. Ni Arc::clone por sí solo ni la existencia de Vec en un objeto prueba asignación por muestra.

**B03 — Publicar el secuenciador clona sus Vec; tap tempo asigna y depende del instante de consumo. CONFIRMADO; alta; confianza alta.**

Evidencia: `src-tauri/src/audio/engine.rs`, `sync_shared_sequencer`, 486–489, comandos 676–681, 836–849; `src-tauri/src/audio/sequencer.rs`, campos 273–274, reservas 312–313, `save_undo_state`, 489–495 y `tap_tempo`, 609–625.

Con undo no vacío, clone copia memoria heap y la asignación al espejo destruye la copia previa. Reservar 16 en el original no vuelve zero-alloc las clonaciones. Tap colecta intervalos en Vec, retiene por ventana temporal sin máximo de entradas y consulta Instant en audio. No todo Instant implica syscall en cada plataforma; la asignación sí es explícita. Además, taps acumulados durante una pausa del stream se fechan juntos al drenarse, perdiendo su separación real. El desplazamiento de hasta 15 BankPattern al remover el primer Undo está acotado: es candidato a optimización, no “bucle infinito”.

**B04 — Trabajo de control sin presupuesto y entrada MIDI sin timestamp de audio. CONFIRMADO; alta para presupuesto, media para granularidad MIDI; confianza alta.**

Evidencia: `src-tauri/src/audio/engine.rs`, `process_commands`, 498–501 y hasta 1107; `src-tauri/src/midi/mod.rs`, callback, 164–170 y 197–257; `src-tauri/src/audio/mod.rs`, `AudioEngineCommand`, 31–88; `src-tauri/src/midi/parser.rs`, 51–54; `record_live_hit`, sequencer.rs:514–516.

El consumidor drena hasta vacío; el productor puede rellenar mientras se consume. Capacidad 1024 no limita el número total procesado en esa invocación. No se demuestra starvation con un teclado concreto; sí ausencia de presupuesto, además de comandos caros.

MIDI descarta `_stamp`, emite a Tauri antes de encolar y toma un mutex del productor. Los comandos carecen de timestamp/offset; live MIDI/QWERTY se aplica en frontera de callback y se graba con el tick del instante de consumo. Es distinto de A01: un evento en vivo no puede reconstruir una muestra pasada sin una política de latencia. No prometer “jitter cero” ni añadir un buffer completo por defecto como supuesta corrección inocua. Tampoco enviar logs, serialización o backpressure al callback.

### C. Rendimiento pendiente de medición

**C01 — Frecuencia de render React/IPC. PARCIALMENTE CONFIRMADO; severidad de rendimiento no determinada.** `useMidiStream.ts:137–139,266–390`, `App.tsx:34–54,281–306,932–1010`. Son demostrables las actualizaciones de raíz y las copias de mapas/historial de 50 eventos; no se ha probado caída de FPS o pérdida de audio. Los pasos a 1/32 y 300 BPM son 40/s, y los eventos fuera de grilla pueden producir notificaciones adicionales (sequencer.rs:781–783). Medir antes de introducir un store, memo generalizado o batching.

**C02 — Coste DSP y reconstrucción. HALLAZGO NUEVO como candidato de medición, no bug.** `fx.rs:122–175,571–630` calcula funciones trascendentes y coeficientes por frame; `engine.rs:1242–1253` renderiza ocho sintetizadores de batería; `AudioRenderState::new` clona el rack (410–414) y luego update_sample_rate reconstruye delay/reverb/chorus (1505; fx.rs:498–503). El primer coste importa al deadline; el último ocurre fuera del callback. No se infiere que ninguno sea el cuello de botella. Medir con build release y parámetros representativos; no alterar suavizados, filtros, paneo o tails para mejorar una cifra sin comparar el audio.

### D. Refactorizaciones opcionales y resultados negativos

- **D01:** separar ownership RT, configuración y ciclo de vida puede ayudar a implementar B01–B03/A16, pero no se recomienda modularizar por tamaño ni ejecutar primero una división masiva de seis archivos. La corrección tiene un criterio funcional independiente de dónde viva el código.
- **D02:** `MidiMonitor` no tiene importador en src; `MidiStreamPopover` y `useMidiStream` sí se usan. `AudioEngine::set_knob_mapping/set_knob_value` y `PadSampler::get_gain` no muestran consumidores de producción dentro del alcance. Es limpieza opcional, no optimización de audio. `PadSampler::update_sample_rate` **sí** se usa (engine.rs:1508,1629).
- **D03:** contratos TS/Rust manuales y alternativas snake_case/camelCase admiten una prueba de serialización. Generación automática o nuevas bibliotecas requieren beneficio concreto; el duplicado entre lenguajes no es por sí solo un bug.
- No se demuestra panic normal por channels=0: la división existe (engine.rs:1335), pero falta un dispositivo válido o frontera que produzca ese valor. Igual para índices de buffers ASIO: hay supuestos no verificados, no un crash acreditado.
- `sampler.rs:103–105` indexa frame[1], pero para atribuir panic a un WAV incompleto habría que demostrar que hound acepta esa estructura y entrega ese resto. Fuera del alcance de dependencias; no se eleva a bug confirmado. `unwrap_or(0)` en decodificación (58–77) sí oculta errores si el iterador los produce, pendiente de fixture válido para esa ruta.
- `unsafe impl Send/Sync` (engine.rs:196–198,242–243) obliga a justificar contratos de plataforma; su presencia no demuestra use-after-free ni data race. La exclusión por Mutex de Producer/Consumer no viola automáticamente SPSC. Hay contención y carrera de ciclo de vida, no prueba de acceso simultáneo al mismo consumer sin guard.
- Un mutex envenenado produce panic en unwrap, pero no se encontró una causa primaria normal que lo envenene. `expect` al crear hilos falla si el sistema rechaza crearlos; no atribuir ese caso a desconectar MIDI. `eprintln!` del callback de error CPAL no fue demostrado ejecutándose en el hilo de render.
**A23 — UP/DOWN trunca la bajada con acordes expandidos grandes. HALLAZGO NUEVO, confirmado; media; confianza alta.** La secuencia UP/DOWN expandida está limitada a 48 entradas (arpeggiator.rs:302–321); para más de 25 notas expandidas se trunca la bajada, sin overflow gracias a `s < 48`. Es una limitación funcional adicional comprobable; si se requiere todo el rango declarado 16×3, usar recorrido por índice/dirección y test de extremos, sin asignación dinámica. No se confunde con un panic.
**A24 — Master se aplica dos veces al sintetizador de respaldo. HALLAZGO NUEVO, confirmado; baja; confianza alta.** El fallback aplica master en `next_sample` (engine.rs:1145) y en salida (1338–1356): ganancia efectiva cuadrática cuando no hay SF2. Es una inconsistencia adicional de nivel; antes de cambiarla, fijar el nivel nominal esperado del fallback. No afecta al camino SF2.

## Fase 2: contraste adversarial completo del informe anterior

El informe original no asigna IDs. Los O siguientes identifican sus afirmaciones por sección y contenido. “Confirmado” no hereda automáticamente la severidad ni aprueba la solución propuesta. Los candidatos sin consumidores se clasifican respecto de esa observación acotada, sin inferir que deban eliminarse.

| ID | Afirmación original | Clasificación y contraste con el código |
|---|---|---|
| O01 | §3.1 Locks dentro del callback | **CONFIRMADO.** B01 demuestra el callback y un contendiente real. No demuestra deadlock universal ni que try_lock sea una solución suficiente. |
| O02 | §3.2 Construcción y sustitución de sintetizadores en audio | **CONFIRMADO** como ruta de construcción/destrucción RT, B02. El número exacto de allocations y coste interno de rustysynth no se verificó. |
| O03 | §3.2 Crecimiento de buffers >4096 | **CONFIRMADO**, B02. El resize es condicional al tamaño, no ocurre en cada callback. |
| O04 | §3.2 Último Arc de PCM liberado en audio | **CONFIRMADO**, B02; control puede retirar primero su referencia y dejar la última al slot. No toda operación Arc clona el PCM. |
| O05 | §3.3 Clone de secuenciador y tap asignan | **CONFIRMADO**, B03. El patrón vacío por sí solo no prueba una asignación, pero historial no vacío y collect sí. Instant no equivale necesariamente a una syscall. |
| O06 | §3.3 Undo pre-reservado no basta para certificar ausencia de realloc | **CONFIRMADO** como cautela. Se clona el contenedor (engine.rs:418–420,486–489), no se mantiene necesariamente su capacidad reservada. No afirmar que cada push de Undo reasigna. |
| O07 | §3.4 Offsets perdidos en arp, secuenciador y clic | **CONFIRMADO**, A01. Distinguir precisión del reloj de precisión de salida; validar también el buffering interno SF2 después de segmentar. |
| O08 | §3.5 Overflow, éxito IPC y NoteOff perdido | **CONFIRMADO**, A02. La suspensión de audio ofrece una ruta concreta para acumular comandos. |
| O09 | §3.5 Drenaje completo puede exceder el presupuesto | **CONFIRMADO** como riesgo RT, B04. La cola puede rellenarse durante el drenaje; no se midió starvation. |
| O10 | §3.6 Desconexión MIDI sin panic/limpieza | **CONFIRMADO**, A04, incluyendo cambio manual de puerto. La duración audible depende de voz, pedal y modo. |
| O11 | §3 Robustez: unwrap propaga mutex envenenado | **PARCIALMENTE CONFIRMADO.** Existe el mecanismo, falta el panic primario. Como bug normal causado por desconexión sería **NO DEMOSTRADO**. |
| O12 | §3 Robustez: expect de creación de hilos | **PARCIALMENTE CONFIRMADO.** El fallo es real si el SO rechaza crear el hilo (engine.rs:1900,2965; midi/mod.rs:396), no observado aquí. Cambiar expect sin política de funcionamiento degradado no basta. |
| O13 | §3 Robustez: logging en callback de error | **PARCIALMENTE CONFIRMADO.** eprintln está en engine.rs:1523–1525; falta demostrar ejecución en hilo RT del backend concreto. |
| O14 | §3 Robustez: reconexión automática ASIO no demostrada | **CONFIRMADO** como limitación de cobertura: engine.rs:2864–2868 omite esa recuperación. No se acredita que todo driver ASIO sea incapaz de recuperarse por sí mismo. |
| O15 | §3 Conversiones i16/u16 y silencio | **CONFIRMADO.** engine.rs:1380–1386,1401–1418: -1→-32767, cero activo u16→32767 y canales vacíos→32768. Diferencia pequeña, sin panic ni defecto musical grave demostrado. |
| O16 | §3 channels=0 causaría panic | **PARCIALMENTE CONFIRMADO** como aritmética; **NO DEMOSTRADO** como entrada alcanzable normal. engine.rs:1477–1478 recibe canales del dispositivo. |
| O17 | §3 Validar finitud y rangos, revisar unsafe ASIO | **NO DEMOSTRADO** como fallo genérico actual: faltan entradas y contratos concretos. A21 sí identifica una rama específica incorrecta condicionada al formato. No inferir corrupción solo por unsafe. |
| O18 | §3 Vec de FX y WAV loader no prueban asignación por muestra | **CONFIRMADO.** fx.rs:204–215,498–503 y sampler.rs:43–155 construyen fuera del render ordinario; render_mix 297–343 usa memoria existente. Mantener esa distinción. |
| O19 | §4 Re-render raíz por MIDI | **CONFIRMADO** el mecanismo; **NO DEMOSTRADO** el cuello de botella. C01. Crear hooks adicionales dentro de App no aislaría el estado de su render. |
| O20 | §4 Re-render por pasos, hasta 40 pasos/s | **CONFIRMADO** el cálculo de pasos a 300 BPM/1-32; no es un límite de todos los eventos: también puede haber triggers fuera de grilla. C01. |
| O21 | §4 Fuga asíncrona de listeners App | **PARCIALMENTE CONFIRMADO**, A20: condición cleanup antes de resolución; no se demuestra remount habitual en producción. La prioridad alta incondicional de la matriz original no se sostiene. |
| O22 | §4 Gestos incompletos | **CONFIRMADO**, A13. Matiz para la solución: ya hay captura de puntero; falta cleanup común y control de identidad/cancelación. |
| O23 | §4 Respuestas de audio obsoletas | **CONFIRMADO** como defecto frontend bajo respuestas tardías, A14; frecuencia real pendiente. No atribuir una carrera de memoria al backend por ello. |
| O24 | §4 Calibración no es latencia exacta de hardware | **CONFIRMADO**, A15. El motor se reanuda antes de medir; sigue siendo otra ruta Web Audio/JS. El original no detectó la dependencia errónea de BPM. |
| O25 | §4 Timeout anidado sin cleanup completo | **PARCIALMENTE CONFIRMADO.** App.tsx:123–129 no conserva el segundo timer. Requiere desmontaje durante esos 700 ms; timer finito, no fuga creciente ni bug musical acreditado. |
| O26 | §2 MidiMonitor sin consumidor; MidiStreamPopover vivo | **CONFIRMADO** dentro de src: Header.tsx:5,205 consume el segundo; no hay import del primero. Retirada opcional D02. |
| O27 | §2 useMidiStream no está muerto | **CONFIRMADO**, App.tsx:11,34–54. Quitar el hook rompe funciones centrales. |
| O28 | §2 Doce IPC sin invoke literal | **CONFIRMADO** como búsqueda acotada en src; **NO DEMOSTRADO** que sean API muerta. Siguen registrados en main.rs:1176–1259. No hay autorización técnica para borrar compatibilidad por esa búsqueda. |
| O29 | §2 AudioEngine::set_knob_mapping/set_knob_value y PadSampler::get_gain sin consumidores de producción | **CONFIRMADO** dentro del alcance. No confundir los métodos de AudioEngine con los métodos de AudioFxRack que sí se llaman. D02. |
| O30 | §2 PadSampler::update_sample_rate sin llamadas de producción | **FALSO POSITIVO.** Llamadas explícitas en engine.rs:1508 (CPAL) y 1629 (ASIO). El borrado propuesto en la matriz original rompería esas llamadas; además dejaría sin actualizar fade. A05 demuestra el defecto distinto: no remuestrea los datos. |
| O31 | §2 Contratos TS/Rust duplicados | **CONFIRMADO** como mantenimiento, no bug por duplicación. state.rs:9–30, Header.tsx:7–19 y src/types. D03. |
| O32 | §2 Doble estado de secuenciador | **CONFIRMADO**, A03. La auditoría independiente demuestra divergencia incluso sin overflow o try_lock fallido. |
| O33 | §2 SPEC referencia mapping.rs/sniffer.rs ausentes | **CONFIRMADO** para el árbol de producción. El parser real está en midi/parser.rs y el sniffer en examples/sniffer.rs. No son dependencias perdidas del motor. |
| O34 | §1 engine.rs: riesgo alto por mezcla de dominios | **NO DEMOSTRADO** por composición/tamaño. Sus bugs RT sí están en B01–B04. Extraer render/stream no los corrige por sí mismo; D01. |
| O35 | §1 main.rs: riesgo medio por frontera grande | **PARCIALMENTE CONFIRMADO** por los dos escritores A03; el tamaño no es una evidencia adicional. Extraer comandos es opcional. |
| O36 | §1 StepSequencer.tsx: riesgo medio-alto por ciclo de render único | **NO DEMOSTRADO** como severidad de rendimiento. La edición A10 tiene causa concreta en el contrato; memorizar filas no la corrige. |
| O37 | §1 AudioSettingsModal: riesgo medio por responsabilidades | **PARCIALMENTE CONFIRMADO** por A14/A15. Extraer hooks no impone secuenciación ni corrige el reloj de medición. |
| O38 | §1 App: riesgo medio-alto por estado raíz | **PARCIALMENTE CONFIRMADO** en estructura; coste C01 pendiente. Sus bugs de listeners/transporte tienen evidencias separadas A18/A20. |
| O39 | §1 HardwareSkeleton: riesgo medio | **PARCIALMENTE CONFIRMADO** por A12/A13, no por líneas de JSX. Extraer FaderBank o Keyboard es opcional. |
| O40 | §5 División por dominios como prioridad media antes de fases futuras | **NO DEMOSTRADO** como intervención necesaria. No hay evidencia que obligue a reestructurar primero ni a añadir stores/capas. Se mantiene D01 como opción subordinada a pruebas y ownership. |

**Hallazgos nuevos respecto del original:** A05–A12, A15 (dependencia del tempo), A16–A19 y A21–A24. A03 añade una demostración más fuerte a un hallazgo existente. B04 añade la pérdida explícita de timestamp MIDI; C02 es solo una línea de medición. A22 se limita a una frontera IPC invocable desde la aplicación, sin afirmar explotación remota.

### Riesgos de las soluciones sugeridas

1. **Consumidor “propiedad del stream”.** Es una dirección razonable, pero mover Consumer al closure sin recuperarlo al parar deja sin consumidor la cola en el siguiente stream. Transferir la misma instancia mutable a dos callbacks sería incorrecto. Primero serializar construcción/parada, definir devolución al propietario de control y confirmar que el callback anterior terminó. No resolverlo con clones lógicos de render activos simultáneamente.

2. **Sintetizadores preparados y reclamación diferida.** Prepararlos con el sample rate antiguo introduce A05 en SF2 si la configuración cambia antes del intercambio. Etiquetar recursos con generación/configuración y rechazar los obsoletos fuera de RT. Una cola de retirada llena no puede causar drop en audio ni espera; reservar cupo antes de aceptar otro intercambio o retener el retirado en almacenamiento preasignado. El comando rechazado también debe destruirse fuera del callback. No asumir que “usar Arc” ya garantiza reclamación no RT.

3. **Cambiar todos los lock por try_lock.** Elimina algunas esperas, pero descarta persistencia o sincronización sin reintento. Puede agravar A03/A06. Los parámetros del render necesitan ownership local; para publicación usar datos pequeños y versiones. Atomizar cada campo independientemente puede mezclar banco, patrón y resolución de distintas revisiones.

4. **Priorizar NoteOff.** Adelantar NoteOff por delante de su NoteOn todavía encolado crea una nueva nota colgada. Preservar causalidad dentro de cada origen/nota; no reordenar arbitrariamente. Si el overflow vuelve indeterminado el estado, un reset por generación debe invalidar también NoteOn pendientes antiguos. Un all-notes-off global puede cortar al secuenciador y a QWERTY al desconectar un único puerto: delimitar alcance.

5. **Backpressure y presupuesto de comandos.** Esperar en el productor MIDI puede retrasar el siguiente NoteOff y la desconexión. Limitar N comandos por buffer sin distinguir controles continuos y eventos discretos añade latencia musical. Coalescer solo setters con semántica de “último valor”; no coalescer toggles, tap, notas, pintura/Undo ni mutaciones cuya secuencia importa. Medir la latencia máxima antes de fijar el presupuesto.

6. **Render por segmentos.** Conserva offsets si todos los buses, el clic y FX recorren la misma línea temporal y el reloj avanza exactamente una vez por frame. No reiniciar efectos ni envelopes en cada segmento ni aplicar master dos veces. Los eventos simultáneos requieren una regla estable de release/trigger. Evitar Vec de eventos por buffer y comprobar el tamaño de bloque interno SF2. Un bucle de render SF2 de una muestra puede multiplicar el coste sin conseguir precisión si la biblioteca mantiene buffering interno.

7. **Snapshots e historial fijo.** Publicar solo SequencerStateInfo basta para dibujar, pero no reconstruye eventos off-grid/original_tick ni Undo. Separar telemetría de estado musical persistente. Una estructura fija enorme copiada en cada NoteOn elimina malloc pero puede aumentar el peor tiempo; publicar solo cambios necesarios. El índice/banco pertenece al snapshot de Undo, no a la UI actual.

8. **Batching React/RAF.** Agrupar indicadores o monitor no debe retrasar comandos musicales, MIDI Learn ni los toggles ARP que hoy nacen en JS. En pestañas ocultas RAF puede no ejecutar con la cadencia esperada. React.memo pierde utilidad si todas sus props cambian; un nuevo store exige medir beneficio y mantener suscripciones con cleanup.

9. **Limpieza y validaciones genéricas.** No borrar update_sample_rate. No reemplazar todos los unwrap por valores por defecto que anuncien audio activo inexistente. Rechazar un formato ASIO desconocido es preferible a reinterpretarlo. Corregir el sesgo de un LSB es de baja prioridad y requiere decidir una convención PCM, no una reescritura completa de conversión.

## Fase 3: matriz de intervención

Abreviaturas de evidencia: E = `src-tauri/src/audio/engine.rs`; S = `src-tauri/src/audio/sequencer.rs`; R = `src-tauri/src/audio/arpeggiator.rs`; P = `src-tauri/src/audio/sampler.rs`; M = `src-tauri/src/main.rs`; MIDI = `src-tauri/src/midi/mod.rs`; SF = `src-tauri/src/soundfont/mod.rs`. Las funciones y mecanismos completos están en las fichas anteriores. Las filas “nuevo” conservan su estado de confirmación.

### A. Funcionales

| ID | Clasificación | Severidad | Evidencia técnica | Consecuencia funcional | Corrección mínima recomendada | Riesgo de regresión | Prueba necesaria |
|---|---|---|---|---|---|---|---|
| A01 | CONFIRMADO | Alta | E:1161–1246,1321–1328; trigger antes del render completo | Ataques/clic adelantados, eventos colapsados | Renderizar hasta cada offset y aplicar evento antes de continuar; sin heap | Alto: orden de eventos, buffering SF2, fase FX | Impulsos PCM/clic y ataques/releases SF2 en offset 0,1,N−1; N=64,256,1024,8192; varios eventos por bloque y mismo audio con distinta partición |
| A02 | CONFIRMADO | Alta | M:1159,275–297; MIDI:197–257; push ignorado | Notas colgadas, edición/carga ficticia | Informar overflow fuera de RT, confirmar aplicación y definir recuperación causal de notas | Alto: reordenar o descartar NoteOn/Off | Llenar cola con stream suspendido; NoteOn antes y NoteOff durante overflow; reanudar sin voz huérfana; no devolver éxito de aplicación falsa |
| A03 | CONFIRMADO | Alta | M:934–952; E:486–489,886–890 | Banco/patrón/UI divergentes sin overflow | Un único escritor musical; respuestas/versiones del estado aplicado; política explícita en suspensión | Alto: latencia UI y edición optimista | Barreras deterministas entre enqueue, callback y espejo; toggle banco/clic, delta BPM y doble Undo |
| A04 | CONFIRMADO | Alta | MIDI:93–97,128–136,362–376 | Voces/pedal/LEDs activos tras perder puerto | Terminar voces y estado del origen; recuperación también al cambiar puerto | Medio: corte de notas ajenas | Desconectar con sustain, Gate, ARP/HOLD; QWERTY y secuenciador simultáneos deben seguir según política |
| A05 | NUEVO, confirmado | Alta | P:225–230,319–340; E:368–376,1508,1629 | PCM cambia afinación/duración | Preparar PCM para nueva tasa fuera de RT desde fuente conservada, intercambio junto a configuración | Medio: calidad de remuestreo y muestras de usuario | Tono/impulso cargado a 48k→44,1k→96k; duración y frecuencia constantes; conservar kit usuario |
| A06 | NUEVO, confirmado | Alta | E:418–420,468–473,1510–1517 | ARP/CC/banco SF2 y transporte cambian al abrir ajustes | Restaurar ajustes completos; capturar transporte al detener; no restaurar voces accidentalmente | Alto: ciclo de vida y notas activas | Abrir/cancelar y hot-plug con ARP, CC personalizados, preset banco≠0 y transporte avanzado; comparar UI/motor |
| A07 | NUEVO, confirmado | Alta | E:532–575,1080–1086; R:338–381,404–406 | Voz anterior sin release al conmutar o acelerar ARP | Liberar/transferir notas directas al activar; preservar release de nota anterior antes de reemplazo | Medio: articulación, sustain/HOLD | Mantener nota y activar; gate largo→1/32 con dos alturas; trazar par NoteOn/Off por voz |
| A08 | NUEVO, confirmado | Media | R:338–356 | Deriva de una muestra por gate | Avanzar reloj también al liberar; poder representar ambos eventos si coinciden | Bajo/medio: evento simultáneo | Medir 10000 intervalos a 120 BPM: 6000 exactos cuando corresponda; BPM fraccionario y cambios de rate |
| A09 | NUEVO, confirmado | Alta | S:274,397–433,489–505,544–548 | Undo pisa otro banco o no recupera Clear/Requantize | Snapshot con banco; registrar cada transacción destructiva una vez | Medio: UX de Undo y memoria fija | A/B distintos, editar A, ir a B y Undo; Clear y Requantize seguidos de Undo; drag como una transacción |
| A10 | NUEVO, confirmado | Media | S:335–350,435–482; App:694–716 | Eventos invisibles o imborrables | Compartir proyección circular y pertenencia de eventos a celdas para borrar | Medio: borrar múltiples eventos próximos | Grabar ticks 23 y 383 con quantize off; cambiar grilla, borrar celda, verificar eventos reales y Undo |
| A11 | NUEVO, confirmado | Alta | parser.rs:143–154; M:613–627; useMidiStream:466–489 | Pad aprendido no dispara; knob virtual mueve otro | Mapa de entrada MIDI separado de nota SF2; controles virtuales por índice | Medio: canales, mapeos antiguos y fallback GM | Aprender nota60 pad1; canal melódico no secuestrado; K2=CC56 y mover K1 virtual; preset de batería conservado |
| A12 | NUEVO, confirmado | Alta | HardwareSkeleton:206–225,897–904; StepSequencer:1060–1063 | Ratón corta nota o Gate del hardware | Liberar únicamente notas/pads pertenecientes al gesto local | Bajo/medio: glissando | Sostener nota/pad físico y entrar/salir sin click; glissando y cancelación siguen liberando sus propias notas |
| A13 | CONFIRMADO | Media | HardwareSkeleton:234–335 | Controles siguen arrastrando; bend no centra | Finalizador idempotente para up/cancel/blur/desmontaje; filtrar pointerId | Bajo | pointercancel y lost capture sin up; no IPC en movimientos posteriores; bend vuelve a 0 una vez |
| A14 | CONFIRMADO en frontend | Media | AudioSettingsModal:191–226,237–374 | Opciones de A asociadas a B | Token por selección/apertura; descartar respuestas obsoletas; ordenar operaciones de lifecycle | Bajo/medio | Resolver B antes de A; cerrar/reabrir durante probe; aplicar solo valores de selección vigente |
| A15 | NUEVO (BPM), confirmado | Media | AudioSettingsModal:412–525,1175–1182; S:514–523 | Compensación física cambia con tempo; medición de otra ruta | Guardar duración física y convertir al tempo actual; documentar/mantener asistente perceptual o medir ruta nativa | Medio: migración de ticks guardados | Offset fijo a 60/120/240 BPM debe representar mismos ms; dispositivo distinto y carga de JS; migración a referencia 120 |
| A16 | NUEVO, confirmado | Alta | E:2128–2140,2319,2766–2819,2864–2944 | Watchdog pisa configuración/suspensión; sin recuperación inicial | Serializar dueño del stream y generación; iniciar vigilancia también sin DAC; invalidar resultados antiguos | Alto: todas las plataformas | Barreras watchdog/manual/Suspend; arranque sin DAC y conexión posterior; nunca dos generaciones activas |
| A17 | NUEVO, confirmado | Media | SF:333–345,460–471; App:378–406 | Carga SF2 distinto o reemplaza importación previa | Identidad origen+nombre validada; política visible de colisión | Bajo/medio: compatibilidad catálogo | Factory y user con mismo nombre y contenido distinto; seleccionar ambos; importar homónimo sin pérdida implícita |
| A18 | NUEVO, confirmado | Media | App:334–342 | Transporte alterna al mantener Espacio | Ignorar repeat en ese listener | Bajo | Un keydown seguido de repeats y keyup produce un toggle |
| A19 | NUEVO, confirmado | Media | S:262–263,528–539,756–768 | Redoble accidental al grabar pads simultáneos | Supresión acotada por pad/tick de esta vuelta | Bajo/medio: no silenciar siguiente compás | Dos pads tick20→24; ambos sin redisparo, ambos suenan en siguiente vuelta |
| A20 | PARCIALMENTE CONFIRMADO | Media condicionada | App:273–331,346–356; main.tsx:7–9 | Listener sobrevive si cleanup precede registro | Cleanup con promesa/bandera; no cambiar estado raíz | Bajo | Desmontar antes de resolver listen; resolver y comprobar unlisten una vez; sin duplicación al remontar |
| A21 | NUEVO, parcial | Alta condicionada | E:1584–1587,1714–1723 | Formato ASIO incorrecto al alcanzar fallback | Rechazar formato no soportado antes de arrancar | Bajo: limita dispositivos hoy mal soportados | Doble de driver con formato desconocido; error limpio sin callback; hardware para formatos explícitos |
| A22 | NUEVO, confirmado en IPC | Alta potencial | M:60–63; SF:504–509 | Borrado fuera de biblioteca desde IPC arbitrario | Exigir componente normal basename y confinamiento; conservar borrado legítimo | Bajo | En directorio temporal aislado: absoluto, parent, symlink y nombre válido; nunca tocar archivos externos |
| A23 | NUEVO, confirmado | Media | R:302–321 | UP/DOWN pierde tramo descendente con expansión grande | Índice y dirección sobre base expandida, sin duplicarla en Vec | Medio: extremos del patrón | 16 notas×3 octavas, alturas válidas, UP/DOWN sin truncar ni duplicar extremos |
| A24 | NUEVO, confirmado | Baja | E:1145,1338–1356 | Master cuadrático en fallback | Aplicar ganancia master una sola vez tras mezcla con nivel nominal acordado | Medio: nivel percibido del fallback | Sin SF2, ratios 1,0/0,5/0,25; camino SF2 y pads conservan nivel y clipping esperado |

### B. Tiempo real

| ID | Clasificación | Severidad | Evidencia técnica | Consecuencia funcional | Corrección mínima recomendada | Riesgo de regresión | Prueba necesaria |
|---|---|---|---|---|---|---|---|
| B01 | CONFIRMADO | Alta arquitectónica | E:507–581,715–776,1911–1931 | Deadline depende de un hilo de control | Eliminar locks bloqueantes del render; estado local y publicación acotada/versionada | Alto: persistencia/ciclo de vida | Contender cada mutex desde control bajo carga; callback no espera ni toma locks; verificar estados |
| B02 | CONFIRMADO | Alta arquitectónica | E:507–518,765–775,1148–1155; P:263–273 | Asignador/destructor puede producir xruns | Preparar recursos fuera de RT, retirada diferida con cupo; scratch fijo y chunking | Alto: lifetime/rate, cola retirada llena | Instrumentar alloc/realloc/dealloc y thread de Drop; cargas/clear repetidos y buffer8192; retirada saturada |
| B03 | CONFIRMADO | Alta arquitectónica | E:486–489; S:273–274,609–625 | Heap y coste variable durante comandos | Publicar snapshot pequeño; tap con timestamp de captura e historial fijo | Medio: tap/Undo/reanudación | Historial lleno, múltiples taps, ediciones; cero asignaciones RT y mismo BPM con distinto retraso de consumo |
| B04 | CONFIRMADO | Alta arquitectónica | E:498–501; MIDI:164–170; mod.rs:31–88 | Ráfagas consumen deadline; MIDI pierde timing original | Presupuesto de control con orden causal; timestamp de captura y política de scheduling explícita | Alto: latencia o pérdida de eventos | Flood continuo+notas; tiempo máximo callback, latencia NoteOff, overflow contabilizado; eventos a distinta fase del buffer |

### C. Medir antes de optimizar

| ID | Clasificación | Severidad | Evidencia técnica | Consecuencia funcional | Corrección mínima recomendada | Riesgo de regresión | Prueba necesaria |
|---|---|---|---|---|---|---|---|
| C01 | Mecanismo confirmado; impacto NO DEMOSTRADO | Pendiente | useMidiStream:139,266–390; App:281–306 | Posible coste UI/IPC; sin caída FPS probada | Medir renders y aislar solo suscripciones costosas; monitor opcionalmente agrupado | Medio: perder notas visuales o acciones ARP | React Profiler en build representativa, percentiles de commits y respuesta con MIDI denso; comprobar entregas discretas |
| C02 | NUEVO candidato; impacto NO DEMOSTRADO | Pendiente | fx.rs:122–175,571–630; E:410–414,1242–1253 | Posible CPU DSP y arranque; sin xrun medido | Perfil primero; evitar optimizar algoritmos auditivamente sin referencia | Alto si se altera DSP | Deadline por buffer/p99/máximo con polifonía+FX; renders de referencia, espectro, tails y cambios de parámetros |

### D. Opcionales

| ID | Clasificación | Severidad | Evidencia técnica | Consecuencia funcional | Corrección mínima recomendada | Riesgo de regresión | Prueba necesaria |
|---|---|---|---|---|---|---|---|
| D01 | Necesidad NO DEMOSTRADA | Opcional | Límites reales E:297–344,1424–1554,2824–2966 | Facilitar ownership, sin arreglo por mover archivos | Extraer solo lo necesario para cambios concretos; no por líneas | Medio: visibilidad, cfg y orden de init | Compilar plataformas y comparar audio/IPC antes y después de un movimiento mecánico |
| D02 | Ausencia de consumidores confirmada en alcance; excepción O30 | Baja/opcional | Header:5,205; E:1975–1987; P:289–295; E:1508,1629 | Limpieza, no menor latencia acreditada | Conservar APIs salvo decisión de compatibilidad; nunca retirar update_sample_rate como muerto | Medio si se borra API | Referencias y llamadas dinámicas; compilación por cfg; smoke IPC |
| D03 | Duplicación de contrato CONFIRMADA | Baja/opcional | state.rs:9–30; Header.tsx:7–19; src/types | Riesgo futuro de deriva | Prueba de serialización y tipo TS común; generación solo si compensa | Bajo/medio | Fixtures camelCase y enums reales; consumidores interpretan mismos campos |

## Orden recomendado y criterios de cierre

1. **Establecer pruebas de salida y estado antes de cambios estructurales:** A01, A03, A05–A09 y A16. Usar render offline y dobles de dispositivo/reloj, evitando depender solo de escucha manual. Fijar comportamiento de gate, sustain, Undo, bancos y reinicio.
2. **Corregir ownership y causalidad conjuntamente:** A02/A03/A04/A16 con B01–B04. No retirar el espejo o cambiar de cola de forma aislada si eso rompe restauración, publicación o respuestas IPC. Introducir cambios pequeños y medibles; comprobar que no aumentan buffers ni latencia nominal.
3. **Corregir la línea temporal:** A01 y A08 con pruebas de partición de buffers; A07 exige release correcto incluso durante cambios de configuración. El evento puede ser exacto en el scheduler y seguir no siéndolo en SF2: verificar ambos.
4. **Restauración y edición:** A05/A06/A09–A12/A19, con estados y samples persistidos de referencia. Los arreglos A18, A20 y A22 son acotados y pueden entregarse independientemente con sus pruebas; su facilidad no reemplaza el trabajo RT.
5. **Plataformas y medición:** ASIO A21 y hot-plug físico en cada backend; después decidir C01/C02. Eliminar archivos o reorganizar módulos queda al final y solo con una razón concreta.

Criterio de cierre RT: ausencia instrumentada de alloc/dealloc y locks bloqueantes en todas las rutas del callback, ownership de recursos comprobado y tiempo de callback bajo el deadline con margen medido en hardware objetivo. Criterio temporal: posición de muestras y secuencia de eventos invariantes frente a partición de buffers, dentro de las limitaciones SF2 explícitamente verificadas. Criterio de concurrencia: estado aplicado y publicado convergen con generación/versión, sin notas huérfanas tras overflow, cambio de puerto o reinicio.

Estas son condiciones para validar una intervención futura. Esta auditoría no certifica que el motor ya las cumpla ni que las correcciones propuestas hayan sido implementadas.
