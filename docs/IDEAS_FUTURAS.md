# Backlog de Ideas Futuras e Incubadora — ImpactSampler Studio

> **Propósito de este documento:** Bloc de notas activo para capturar ideas, mejoras puntuales y conceptos en evaluación antes de que pasen a desarrollo.  
> **Regla de limpieza:** Una vez que una idea se implementa en producción, **se elimina de este archivo** (su historial queda registrado en [`PLAN_DE_TRABAJO.md`](../PLAN_DE_TRABAJO.md)).

---

## 1. 🎹 Instrumentos, SoundFonts & Licencias Abiertas

- [ ] **Reemplazo de SoundFonts de Piano y Rhodes por alternativas 100% Open License (Musical Artifacts):**
  - Sustituir `Piano - Realtime.sf2` y `Rhodes (from Dream Piano).sf2` por instrumentos de alta calidad con licencia **CC BY 4.0 / CC0 / GPL** provenientes de [Musical Artifacts](https://musical-artifacts.com/).
  - Al realizar el cambio, incorporar en `README.md` y `soundfonts/README.md` la tabla oficial de atribuciones (TASL: Título, Autor, Fuente y Licencia) junto a los actuales:
    - **Mellotron M400** (`Mellotron.sf2`) — [Musical Artifacts #500](https://musical-artifacts.com/artifacts/500) ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)).
    - **Yamaha RX7** (`Yamaha_RX7_Drums.sf2`) — Autor: *Reza Chaniago Hartono W. Walandouw Singh Khadafi* — [Musical Artifacts #885](https://musical-artifacts.com/artifacts/885) ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)).

---

## 2. 🎛️ Audio, DSP & Expresión Musical

- [ ] **Expansión de Modos Avanzados en el Arpegiador Melódico (`arpeggiator.rs`):**
  - Añadir a los modos actuales (`UP`, `DOWN`, `UP/DOWN`) las variantes de estudio:
    - `AS PLAYED`: Respeta el orden exacto en que los dedos presionaron las notas del acorde.
    - `RANDOM`: Selección aleatoria de las notas sostenidas.
    - `CHORD` (*Poly-Step / Gater*): Dispara el acorde completo rítmicamente a la división de tempo elegida.
  - Evaluar control ajustable de **Gate Length** (`20%` a `100%`), **Swing** (`0%` a `75%`) y divisiones en tresillos (`1/8T`, `1/16T`).
- [ ] **Ajuste Dinámico de Velocidad en Modo Teclado QWERTY:**
  - Evaluar atajos opcionales (`C` / `V`) para atenuar o incrementar la velocidad de pulsación cuando se interpreta desde el teclado de la computadora.

---

## 3. 🥁 Creación de Sonido & Gestión de Sesiones (Apuntes de Diseño para Fases 10–12)

- [ ] **Drum Kit Creator (`WAV Pool` a compilador `.sf2` — Fase 12):**
  - Arrastrar una carpeta de muestras `.wav`, auto-asignar los 8 pads principales por heurística de nombres (*Kick, Snare, Clap, Closed HH, Open HH, Toms, Crash*) y permitir reasignar cualquier muestra del pool mediante clic derecho.
  - Compilar en Rust (`sf2_builder`) un archivo `.sf2` estándar (chunks RIFF `INFO`, `sdta`, `pdta`) guardado directamente en la `User Library`.
- [ ] **Sistema de Sesiones `.impact` y Exportación WAV/Stems (Fase 11):**
  - Guardar estado completo de bancos A/B, BPM, muestras WAV, SoundFont activo, knobs FX y faders en un archivo `.impact`.
  - Renderizado offline en Rust a 24-bit / 48 kHz (Master Loop WAV y 8 Stems separados por pista).
- [ ] **Wizard Interactivo de Calibración de Hardware Universal (Fase 10):**
  - Asistente paso a paso para detectar rango de teclas (25/32/49/61/88), pads, faders y knobs de cualquier controlador externo y guardarlo como perfil reutilizable.

---

## 4. 🚀 Distribución & Ecosistema (Apuntes de Diseño para Fases 13–14)

- [ ] **Auto-Actualizaciones OTA con `@tauri-apps/plugin-updater` (Fase 13):**
  - Comprobación silenciosa al inicio contra `latest.json` en GitHub Releases con indicador discreto en el `Header` y botón manual en `AudioSettingsModal.tsx`.
  - Preservación total de `audio_preferences.json`, `localStorage` y `User Library`.
- [ ] **Formato Plugin VST3 / CLAP para DAWs (Fase 14):**
  - Versión instrumento virtual (`nih-plug`) para abrir dentro de Reaper, Ableton Live, FL Studio o Bitwig, con sincronización bidireccional de BPM/transporte y exposición de los 8 knobs y 9 faders a las pistas de automatización del DAW.
- [ ] **Compilación Nativa Móvil (Android Oboe/AAudio & iOS/iPadOS CoreAudio — Fase 14):**
  - Adaptación de la edición *touch-first* (ya operativa en `web-app/`) a paquete móvil nativo con soporte para controladores MIDI por USB-C.
