<div align="center">
  <img src="branding/iconoIS.png" alt="ImpactSampler Studio Logo" width="96" />
  <h1>ImpactSampler Studio</h1>
  <p><strong>Estación de trabajo musical de baja latencia con motor SoundFont (.sf2), Sampler PCM, Secuenciador por Pasos y Rack DSP en tiempo real.</strong></p>
  <p>
    <img src="https://img.shields.io/badge/Versi%C3%B3n-1.0.0-10b981?style=flat-square" alt="Versión 1.0.0" />
    <img src="https://img.shields.io/badge/Plataformas-Linux%20%7C%20Windows%20%7C%20macOS-06b6d4?style=flat-square" alt="Plataformas" />
    <img src="https://img.shields.io/badge/Core-Tauri%20v2%20%2B%20Rust-f59e0b?style=flat-square" alt="Tauri v2 + Rust" />
    <img src="https://img.shields.io/badge/UI-React%20%2B%20TypeScript-38bdf8?style=flat-square" alt="React + TypeScript" />
  </p>
</div>

---

## Descripción General

**ImpactSampler Studio** es una aplicación de escritorio de audio en tiempo real diseñada con estética **Noir-Tech de hardware de estudio**. Combina síntesis polifónica basada en muestras reales (`.sf2`), un sampler rítmico para 8 pads, un secuenciador por pasos de precisión *sample-accurate*, un arpegiador melódico y un rack modular de efectos DSP escrito íntegramente en **Rust** sin asignaciones de memoria en el hilo de audio (*zero-alloc* / *lock-free*).

Está precalibrada de fábrica para el controlador **Nektar Impact LX49+** (49 teclas, 8 pads, 9 faders, 8 knobs y botones de transporte), pero funciona con cualquier controlador del mercado mediante **MIDI Learn Universal** o directamente con el **teclado de la computadora (QWERTY)**.

---

## Características Principales

* **🎹 Motor Híbrido SoundFont 2 & Librería Extensible:**
  * 4 instrumentos de estudio integrados de fábrica: **Grand Piano** (Steinway), **Rhodes Mark I**, **Mellotron M400** (12 cintas vintage) y **Yamaha RX7 Drum Machine**.
  * Soporte para **Librería de Usuario** (carga de archivos `.sf2` personalizados) y selector rápido en pantalla OLED.
* **🥁 Sampler de 8 Pads & Tres Modos Rítmicos:**
  * Modos **WAV Sampler** (muestras `.wav` propias por Drag & Drop o menú contextual, con disparo *One-Shot* o *Gate*), **Drum SF2** y **Mirror**.
* **⏱️ Secuenciador Rítmico & Arpegiador Sample-Accurate:**
  * Reloj interno en Rust a **96 PPQN** con 0 jitter, bancos **A / B**, resoluciones **1/8, 1/16 y 1/32** (hasta 32 pasos), grabación *Overdub* cuantizada, *Paint / Erase Drag* continuo con **Undo** y metrónomo acústico/visual.
  * **Arpegiador melódico** independiente con modos `UP`, `DOWN`, `UP/DOWN`, función `HOLD` y 1 a 3 octavas.
* **🎛️ Rack DSP de Efectos & Mezclador en Tiempo Real:**
  * **Filtro Bipolar DJ** (LPF / HPF TPT de 12 dB/oct con bypass central y resonancia), **Warm Tube Overdrive** con compensación automática de ganancia (*Auto-Gain*), **Reverb Estéreo**, **Delay**, **Chorus** y **Paneo**.
  * Ruteo selectivo instantáneo (`KEYS | BOTH | PADS`) y mezclador de 9 faders (8 canales de pads + Master).
* **⚡ Audio & MIDI de Baja Latencia con Hot-Plug:**
  * **Linux:** ALSA directo a hardware (`plughw`), PipeWire y JACK.
  * **Windows:** Drivers **ASIO** nativos (con apertura de panel de control de hardware) y **WASAPI**.
  * **macOS:** **CoreAudio** nativo (Universal Binary Apple Silicon & Intel).
  * Reconexión automática en caliente (*Watchdog Hot-Plug*) tanto para interfaces de audio como para cables USB MIDI.

---

## Comandos Útiles

### Desarrollo y Pruebas Diarias

| Comando | Descripción |
| :--- | :--- |
| `npm install` | Instala las dependencias del frontend (Node.js). |
| `npm run tauri dev` | Inicia la aplicación en modo desarrollo con recarga en caliente (*hot-reload*). |
| `npm run update:local` | **Actualización rápida en Linux:** Compila el binario de producción (Rust + React) sin re-empaquetar los 800 MB de SoundFonts y actualiza al instante `/usr/bin/impact_sampler`. |
| `npm run build:bin` | Compila únicamente el binario optimizado en `src-tauri/target/release/impact_sampler` (`--no-bundle`). |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Ejecuta la suite de pruebas unitarias del motor de audio, secuenciador y arpegiador en Rust. |

### Empaquetado de Instaladores de Distribución

| Plataforma | Comando | Salida Generada |
| :--- | :--- | :--- |
| **Linux (`.deb`)** | `npx tauri build --bundles deb` | `src-tauri/target/release/bundle/deb/*.deb` |
| **Linux (`.AppImage`)** | `npx tauri build --bundles appimage` | `src-tauri/target/release/bundle/appimage/*.AppImage` |
| **Windows (`.exe`)** | `npx tauri build --bundles nsis` | `src-tauri/target/release/bundle/nsis/*-setup.exe` |
| **macOS (`.dmg`)** | `npx tauri build --bundles dmg` | `src-tauri/target/release/bundle/dmg/*.dmg` |

> **Nota para actualizar paquete `.deb` completo en Linux:**  
> No es necesario desinstalar la versión previa; basta con ejecutar `sudo dpkg -i src-tauri/target/release/bundle/deb/*.deb`.

---

## Requisitos del Entorno de Desarrollo

* **Node.js** `v18+` y `npm`
* **Rust** (`rustup`, `cargo` `1.77+`)
* **Dependencias de sistema en Linux (Ubuntu / Debian):**
  ```bash
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev
  ```

---

## Estructura del Proyecto

```text
ImpactSamplerStudio/
├── branding/               # Iconografía oficial y recursos gráficos de marca
├── samples/                # Kit de fábrica de 8 samples WAV de percusión
├── soundfonts/             # Bancos de instrumentos SoundFont (.sf2) de fábrica
├── src/                    # Frontend React + TypeScript + Tailwind CSS
│   ├── components/         # Consola HardwareSkeleton, StepSequencer, PerformanceBay, Modals
│   ├── config/             # Catálogo declarativo de instrumentos y perfiles
│   └── hooks/              # Hooks de entrada QWERTY y utilidades de UI
├── src-tauri/              # Backend nativo en Rust (Tauri v2)
│   └── src/
│       ├── audio/          # Motor DSP (engine, sequencer, arpeggiator, sampler, fx)
│       ├── midi/           # Subsistema MIDI, mapeo Nektar LX49+ y Watchdog Hot-Plug
│       └── soundfont/      # Cargador y escáner multi-tier de archivos .sf2
├── ImpactSampler Studio - SPEC.md  # Especificación técnica y arquitectura del sistema
└── PLAN_DE_TRABAJO.md              # Hoja de ruta e historial detallado por fases
```

---

## Documentación Adicional

* 📄 **[Especificación Técnica (`ImpactSampler Studio - SPEC.md`)](ImpactSampler%20Studio%20-%20SPEC.md):** Detalle del mapeo físico MIDI, arquitectura de hilos *lock-free* y diseño del motor de audio.
* 🗺️ **[Plan de Trabajo y Hoja de Ruta (`PLAN_DE_TRABAJO.md`)](PLAN_DE_TRABAJO.md):** Historial completo de fases implementadas y próximos hitos del proyecto.
