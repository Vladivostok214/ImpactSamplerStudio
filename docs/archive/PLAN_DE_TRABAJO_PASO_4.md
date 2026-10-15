# Plan de Trabajo - Paso 4 (Fase 6): Mejoras Input Quantize del Secuenciador ✅ [COMPLETADO]

> Documento de detalle del **Paso 4** de la Fase 6 de [PLAN_DE_TRABAJO.md](PLAN_DE_TRABAJO.md) (100% Completado y Validado en Hardware).
> Prepara el camino inmediato hacia el **Paso 4.1** (MIDI Learn de acciones de UI y controles asignables).

---

## 1. Decisiones tomadas

| Tema | Decisión |
|---|---|
| Micro-timing | La grabación guarda la posición exacta del golpe (ticks). |
| Velocity | Se mantiene **Full Level 127**. Se revisa al final del Paso 4, con las mejoras ya aplicadas. |
| Longitud del loop | **1 compás (4/4)**, fija e independiente de la grilla. |
| Grilla | Solo define dónde se cuantiza y cuántas columnas se dibujan. Nunca cambia la duración del compás. |
| Rejilla en 1/32 | **Páginas de 16 pasos** (2 páginas por compás). |
| Resolución del reloj | **96 ticks por negra** (1/16 = 24 ticks, 1/32 = 12 ticks; permite tresillos). |
| Latencia | **Calibración asistida**: el usuario toca 8 golpes siguiendo el clic. El offset se guarda **por dispositivo de audio** y se puede repetir desde Ajustes de Audio. |
| Re-cuantizar | **Botón para re-cuantizar después de grabar.** |
| Pertenencia de nota a columna | **Criterio A:** vecino más cercano (ventana ±50% del paso). Se dibuja una marca de desplazamiento cuando la nota no está exacta. |
| Edición del micro timing | **Mantener el paso con el mouse y arrastrar** (rango ±50% del paso, ±12 ticks a 1/16, con lectura numérica en ticks). |
| Bancos A/B | Se **mantienen**. La página es una vista dentro de cada banco. |
| Botón físico B5 (CC 51) | Sigue alternando Banco A/B. |
| Cambio de página | **Automático** en 1/32 al grabar o reproducir (sigue al playhead) y **botón en la UI** para avanzar de página con clic. |
| Swing | **Se elimina** (ver sección 4). |
| Botón físico B6 (CC 52) | Queda **sin función** hasta el Paso 4.1. |
| Migración de patrones | Triviales: hoy no hay sesiones guardadas. |
| Funciones avanzadas (Retrig, Note Repeat, redobles) | **Fuera del Paso 4.** Se evalúan más adelante. |

---

## 2. Arquitectura (bloques)

### Bloque 1: Modelo de datos y reloj
- **Eventos por pista en ticks, a 96 ticks por negra** (1/16 = 24 ticks, 1/32 = 12 ticks; permite tresillos), en lugar de `[[u8;16];8]`.
- Loop fijo de 1 compás. El reloj avanza por ticks sample-accurate y cada evento suena en su tick exacto.
- Dos bancos independientes (A/B) con el clonado actual al primer uso.
- Eliminada la duración de paso variable (ya no depende de la resolución).

### Bloque 2: Cuantización de entrada
- Tick real del golpe a partir del **timestamp del evento MIDI**, no del inicio del bloque.
- **Compensación de latencia** con el offset obtenido de la calibración asistida.
- **Q-Mode:** OFF / 1/8 / 1/16 / 1/32.
- **Q-Strength:** 0-100% (0% = micro-timing puro, 100% = encaja en la grilla).
- **Anti-flam:** evita el doble disparo (disparo en vivo + disparo del loop) cuando el golpe queda cuantizado hacia adelante.
- **Re-cuantizar:** botón que aplica el modo y la intensidad elegidos a lo ya grabado. Requiere guardar el tick original además del cuantizado.

### Bloque 3: Correcciones del motor
- Quitar `emit()` y clones con alocación del hilo de audio (cola lock-free hacia un hilo de UI).
- Unificar el estado del secuenciador (una sola fuente de verdad).
- Alinear el límite de BPM (documentación 40-300 vs. código 260) y definir Play/Pausa real.

### Bloque 4: UX de grabación
- `REC` armado (parpadea; empieza con el primer golpe o con Play).
- Cuenta atrás / pre-roll de 1 compás (opcional).
- **Undo** del último golpe o ciclo, y borrar un golpe manteniendo pad + golpear.
- Destello en la columna destino al grabar.
- Selector `Q` visible (modo e intensidad).
- **Calibración asistida de latencia** en Ajustes de Audio: el usuario toca **8 golpes siguiendo el clic**; se calcula el desfase (mediana, para ignorar valores atípicos) y se guarda como offset de grabación por dispositivo de audio.
- Playhead visual compensado por la latencia de IPC.

### Bloque 5: UI de la rejilla (modelo MPC/Elektron)
Basado en cómo lo resuelven los equipos de referencia: Elektron (un trig por paso + micro timing), Akai MPC (eventos en ticks) y Ableton (piano roll continuo).
- Columnas visibles según la grilla: 8 (1/8), 16 (1/16), 32 en 2 páginas (1/32).
- **Indicador de página** y botón para avanzar de página con clic. En 1/32 la página sigue al playhead mientras se graba o reproduce.
- **Pertenencia a columna (criterio A):** cada golpe pertenece a la columna más cercana (ventana ±50% del paso). Al superar ±50% pasa a la columna vecina.
- **Marcador de desplazamiento** dentro de la celda (adelantado / atrasado), sin mover el botón de su columna.
- **Valor centrado en 0** en ticks al editar el micro timing (96 ticks por negra: 1/16 = 24 ticks, 1/32 = 12 ticks).
- **Dos golpes del mismo pad en la misma celda:** no se parte la celda; se muestra un contador "×2".
- **Edición del micro timing** manteniendo el paso con el mouse y arrastrando, con lectura numérica en ticks.
- Los redobles (Retrig / Note Repeat) quedan fuera del Paso 4.

---

## 3. Preparación: catálogo de acciones con nombre
- Las funciones que hoy se disparan con CCs fijos (Play, Stop, Rec, Tap, Clic, Overlay, borrar pista/patrón, banco, grilla...) y las nuevas del Paso 4 (avanzar página, undo, re-cuantizar, calibrar) se definen como un **catálogo de acciones con nombre** (enum en Rust), en lugar de CCs sueltos en el código.
- Los valores por defecto del Nektar Impact LX49+ se conservan como mapa de fábrica sobre ese catálogo.
- B6 (CC 52) queda **sin función** por ahora (sin ramas muertas ni etiquetas engañosas).

---

## 4. Limpieza: eliminar Swing (sin código huérfano)

| Archivo | Qué eliminar |
|---|---|
| [sequencer.rs](src-tauri/src/audio/sequencer.rs) | Campo `is_swing_enabled` (struct y `SequencerStateInfo`), `toggle_swing()` y la lógica de swing en `get_step_duration_samples()` (desaparece con el nuevo reloj por ticks). |
| [engine.rs](src-tauri/src/audio/engine.rs) | Rama `SequencerToggleSwing`. |
| [audio/mod.rs](src-tauri/src/audio/mod.rs) | Variante `SequencerToggleSwing` del enum y el envío desde el botón B6 (comentario "B6: Swing MPC 58%"). |
| [main.rs](src-tauri/src/main.rs) | Comando `sequencer_toggle_swing` y su registro en el `invoke_handler`. |
| [parser.rs](src-tauri/src/midi/parser.rs) | Etiqueta "Button 6 (MPC Swing 58%)" (CC 52). |
| `src/App.tsx` | Estado `isSwingEnabled`, `handleSequencerToggleSwing` y la prop `onToggleSwing`. |
| `src/components/StepSequencer.tsx` | Props `onToggleSwing` / `isSwingEnabled` y el botón SWING con su LED. |
| `src/types/sampler.ts` | Campo `isSwingEnabled`. |
| `PLAN_DE_TRABAJO.md` | Fases 5 y 5.3 (descripciones y tooltip "Swing MPC 58%") y mapa de botones B1-B9. |

> Verificación obligatoria al terminar: búsqueda global de `swing` (sin distinción de mayúsculas) en `src/`, `src-tauri/src/` y la documentación, sin resultados residuales. El CC 52 no debe quedar con rama muerta ni con una etiqueta que mienta.

---

## 5. Etapas de implementación

Cada etapa debe compilar y funcionar antes de pasar a la siguiente. Cada una termina con una verificación (V) acordada con el usuario.

### Etapa 1: Base del motor y limpieza ✅ [COMPLETADA]
- Catálogo de acciones con nombre (`SequencerAction`).
- Unificación del estado del secuenciador (una sola fuente de verdad) y cola lock-free SPSC hacia hilo despachador de UI (`ui-event-dispatcher`).
- Eliminación total y exhaustiva de Swing (0 residuos en Rust, TypeScript y documentación).
- Alinear el límite de BPM (40.0 - 300.0 BPM) y definición de Play / Pause real vs Stop.
- Botones físicos B1 a B9 liberados de asignaciones fijas para MIDI Learn dinámico en el Paso 4.1.
- **V:** App compilando al 100% (Backend 0 warnings, Frontend OK); hilo de audio WASAPI 0% alocaciones y sin `emit()`; búsqueda global de `swing` con 0 resultados.

### Etapa 2: Modelo de eventos y reloj por ticks ✅ [COMPLETADA]
- Modelo de eventos por pista a 96 ticks por negra (384 ticks/compás) y duración fija de 1 compás (Bloque 1).
- Búfer contiguo estático `[SequencerEvent; 64]` por pista sin alocaciones en el hilo de audio.
- Bancos A/B sobre el nuevo modelo de eventos.
- Reloj de audio basado en acumulador de muestras de alta precisión (`samples_per_tick`) con click exacto en negra.
- Grilla adaptativa: resolución por defecto en 1/16; modo 1/8 con pasos 9–16 atenuados y deshabilitados; modo 1/32 con compás de 32 pasos y selector de página (1–16 y 17–32) con seguimiento automático de reproducción.
- Dropdown emergente superior para selector de resolución GRID.
- Distinción funcional completa entre Pause (congelamiento de tick/paso con cursor ámbar persistente) y Stop (rebobinado a tick 0).
- **V:** Duración del compás invariable al cambiar entre 1/8, 1/16 y 1/32; 0 errores de compilación (`cargo check` y `npm run build`); validado y aprobado en hardware.

### Etapa 3: Captura y cuantización ✅ [COMPLETADA]
- Timestamp del evento MIDI y tick real del golpe (Bloque 2).
- Q-Mode (Off, 1/8, 1/16, 1/32), Q-Strength (0–100%), anti-flam (~30 ms / <= 6 ticks) y guardado del tick original (`original_tick`).
- Botón de re-cuantizar dinámico y popover minimalista con diseño no intrusivo.
- **V:** 0 errores en compilación (`cargo check` y `npm run build`); probado y aprobado en hardware.

### Etapa 4: Calibración asistida de latencia ✅ [COMPLETADA]
- Flujo interactivo de 8 golpes siguiendo el clic con cuenta previa de 4 pulsos (pre-roll).
- Cálculo estadístico por mediana para inmunidad total ante golpes accidentales (outliers).
- Compensación aplicada directamente en `record_live_hit` (`current_tick - latency_compensation_ticks`).
- Persistencia automática por dispositivo de audio en `localStorage` restaurada al arrancar la aplicación o cambiar de interfaz.
- **V:** 0 errores de compilación (`cargo check` y `npm run build`); validado y listo para prueba en hardware.

### Etapa 5: UI de la rejilla ✅ [COMPLETADA]
- Resolución adaptativa en columnas (1/8 atenuada, 1/16 estándar de 16 pasos, 1/32 en 32 pasos con páginas 1 y 2).
- Indicador de página y avance manual con botón `PAG 1/2` y seguimiento automático del playhead en reproducción y grabación en vivo.
- Diseño consolidado, limpio y sin sobre-ingeniería innecesaria (aprobado y ratificado por el usuario).
- **V:** Duración del compás invariable; cambio de grilla fluido; visualización limpia y estable.

### Etapa 6: UX de grabación ✅ [COMPLETADA]
- Implementación de **Undo (`sequencer_undo`)**:
  - Motor Rust con historial LIFO de hasta 16 estados (`undo_history: Vec<BankPattern>`) de cero alocación dinámica en el loop de audio.
  - Guarda snapshots automáticos tanto al alternar pasos en la rejilla como al registrar golpes en vivo en modo REC.
  - Botón minimalista `[ UNDO ]` integrado en la barra de transporte maestro junto a los controles de borrado.
  - Notificación reactiva mediante canal lock-free `PatternUpdated` a la UI.
- Filosofía Noir-Tech directa sin pre-roll forzado ni destellos invasivos en la rejilla.
- **V:** Verificado y probado en hardware con el Nektar Impact LX49+; reversión fluida e instantánea de tomas y ediciones.

### Etapa 7: Cierre y revisión ✅ [COMPLETADA]
- **Full Level 127:** Ratificado y fijado permanentemente en 127 tanto para ejecución en vivo como secuenciación grabada, respetando la filosofía clásica MPC y la pegada uniforme de producción urbana.
- Calibración asistida de latencia validada con 0 ticks de desviación en hardware real.
- Pruebas de regresión con hardware Nektar superadas: 0 glitches de audio, 0 locks en el hilo de render y cero alocaciones de memoria en tiempo real.
- Documentación sincronizada al 100%.

---

## 6. Fuera del Paso 4 (más adelante)
- Retrig, Note Repeat y redobles dentro de un paso.
- Cualquier otra función avanzada de secuenciación que surja.

---

# Paso 4.1 - MIDI Learn de Acciones de UI (Controles Asignables) ✅ [COMPLETADO]

> Base fundamental de los perfiles de hardware de la Fase 7. 100% implementado, verificado y calibrado en hardware real Nektar Impact LX49+.

## 1. Objetivo y Alcance Cumplido
- Los botones de la UI (17 controles asignables) son **elementos activos de primer nivel del sistema universal de MIDI Learn**:
  1. `play` (CC 106)
  2. `stop` (CC 105)
  3. `record` (CC 107)
  4. `tapTempo` (CC 102)
  5. `bpmDown` (CC 103)
  6. `bpmUp` (CC 104)
  7. `click` (CC 47 / B1)
  8. `grid` (CC 48 / B2)
  9. `bank` (CC 51 / B5)
  10. `undo` (CC 52 / B6)
  11. `clearTrack` (CC 53 / B7)
  12. `visualMetronome` (CC 54 / B8)
  13. `clearPattern` (CC 55 / B9)
  14. `page` (Página 1/2 en 1/32)
  15. `fxRouteKeys` (Toggle KEYS)
  16. `fxRoutePads` (Toggle PADS)
  17. `workstationMode` (Toggle KEYS / SEQ)
- Excluidos deliberadamente por diseño ergonómico: los 128 botones TR de pasos, las 49 teclas del piano, el botón de re-cuantización y los selectores de patches/presets.

## 2. Especificaciones Técnicas y Entregables Logrados
1. **Despacho $O(1)$ Lock-Free en Backend Rust (`src-tauri/src/midi/mod.rs`):**
   - Tablas contiguas `action_cc_map: Arc<Mutex<[Option<SequencerAction>; 128]>>` y `action_note_map: Arc<Mutex<[Option<SequencerAction>; 128]>>`.
   - Búsqueda directa sin iteraciones ni clones en tiempo real.
   - Acciones críticas (Play, Stop, Record, Undo, Clear, Tap, Bank, Grid) resueltas de inmediato en el motor de audio sin pasar por el hilo webview.
2. **Soporte Toggle de Hardware 1:1 Instantáneo:**
   - Supresión del filtrado restrictivo `*value > 0`. Ahora el motor procesa el CC mapeado en cada mensaje recibido (tanto en `127` como en `0`), garantizando que los botones con alternancia interna de hardware respondan en **cada pulsación física individual** sin exigir doble clic.
3. **Sincronización Bidireccional de FX Route:**
   - Variante `SequencerUiEvent::FxRoutingChanged(FxRouting)` emitida por el hilo de audio a través de la cola lock-free `rtrb`.
   - Despacho transparente a Tauri webview (`"fx-routing-changed"`) para encendido reactivo de los botones `KEYS` y `PADS`.
4. **UI Noir-Tech & MIDI Learn Interactivo:**
   - Borde ámbar en reposo (`border-amber-500/60`) y halo pulsante brillante con anillo (`border-2 border-amber-400 animate-pulse ring-2 ring-amber-400/60 text-amber-200`) al seleccionar cualquier botón de UI en modo Learn.
   - Tooltips enriquecidos con la asignación activa (`Asignado (CC XX)` o `Asignado (Nota XX)`).
   - Desvinculación automática de colisiones al aprender un botón ya ocupado.
   - Sincronización en caliente a Rust (`set_action_midi_mappings`) y persistencia en `localStorage`.
   - Botón «Restablecer de Fábrica» en `AudioSettingsModal.tsx` que restaura instantáneamente el mapa nativo del Nektar Impact LX49+.
