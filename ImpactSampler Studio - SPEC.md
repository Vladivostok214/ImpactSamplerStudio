# **ImpactSampler Studio \- Especificación Técnica v1.0**

## **1\. Visión y Fronteras**

> * **Objetivo del Sistema:** Aplicación de escritorio de baja latencia para Windows que ingesta el flujo MIDI unidireccional de un controlador Nektar Impact LX49+, mapea su matriz de controles (teclado, 8 pads, 8 knobs, 9 faders, transporte) y dispara/modula samples de audio (.wav) cargados en memoria, enrutables a las interfaces de salida del sistema.  
> * **Alcance del MVP:**  
  * Enumeración y selección de puertos MIDI In físicos en Windows.  
  * Spike / Sniffer interactivo de calibración para identificar y mapear los identificadores MIDI (Note / CC) del LX49+.  
  * Carga de archivos .wav (16/24-bit PCM) en memoria RAM asociados a pads/teclas.  
  * Motor de audio estéreo polifónico (hasta 16 voces concurrentes) con control de volumen/ganancia por voz y global.  
  * Selección de dispositivo de salida de audio de Windows (WASAPI).  
  * UI moderna con visualizador reactivo de pads/faders/knobs y asignación de samples mediante arrastrar y soltar (Drag & Drop).  
> * **Fuera de Alcance (Out of Scope):**  
  * Retroalimentación MIDI bidireccional (LEDs / SysEx al hardware).  
  * Efectos DSP complejos (reverb convolutiva, delay tempo-sync, compresor multibanda).  
  * Modos DAW propietarios de Nektar (cubierto exclusivamente bajo modo MIDI Generic / CC estándar).  
  * Grabación o exportación de audio a disco (DAW timeline / render).

## **2\. Stack Tecnológico y Restricciones**

> * **Core / Runtime:** Rust (edición 2021\) ejecutándose sobre Tauri v2.  
> * **Librerías Clave:**  
  * **MIDI:** midir (backend Windows MM / UWP).  
  * **Audio Engine:** cpal (WASAPI Exclusive/Shared), symphonia o hound para decodificación rápida de PCM .wav.  
  * **Concurrencia / Lock-Free:** rtrb (Ringbuffer Single-Producer Single-Consumer) para comunicación sin bloqueos (lock-free) entre hilo MIDI/UI y el hilo de render de audio.  
  * **Frontend UI:** Vite \+ React \+ TypeScript \+ Tailwind CSS \+ Lucide Icons.  
  * **IPC / Estado UI:** Comandos y Eventos de Tauri v2 (tauri::emit, tauri::command).  
> * **Restricciones No Funcionales:**  
  * **Latencia Objetivo:** ≤ 8 ms (buffer de 128 a 256 samples @ 44.1kHz / 48kHz en WASAPI).  
  * **Garantías de Audio Thread:** Cero asignaciones dinámicas de memoria (alloc), cero llamadas de sincronización bloqueante (Mutex, locks de SO) dentro del callback de procesamiento de audio de cpal.  
  * **Consumo de Memoria:** \< 80 MB base (excluyendo el tamaño directo de los samples cargados en RAM).  
  * **Compatibilidad de SO:** Windows 10 / 11 (x86\_64).

## **3\. Arquitectura del Sistema**

### **Flujo de Datos**

`[ Nektar LX49+ ]`  
       `│`  
       `▼ (USB MIDI Stream)`  
`┌────────────────────────────────────────────────────────┐`  
`│ Rust: midir Callback Thread                            │`  
`│  - Decodificación Byte stream -> MidiRawEvent          │`  
`│  - Normalización -> EngineEvent                        │`  
`└──────────────┬──────────────────────────┬──────────────┘`  
               `│                          │`  
        `(rtrb RingBuffer)          (Tauri Event Channel)`  
               `│                          │`  
               `▼                          ▼`  
`┌──────────────────────────────┐   ┌──────────────────────────────┐`  
`│ Rust: Audio Thread (cpal)    │   │ Frontend UI (WebView)        │`  
`│  - Voice Allocator (16 voces)│   │  - Matriz visual reactiva    │`  
`│  - Mixdown PCM Buffer        │   │  - Asignador de samples      │`  
`│  - Master Gain               │   │  - Selector de Audio Device  │`  
`└──────────────┬───────────────┘   └──────────────────────────────┘`  
               `│`  
               `▼`  
`[ Windows WASAPI Audio Device ]`

### **Estructura de Directorios**

`impact-sampler/`  
`├── src-tauri/`  
`│   ├── Cargo.toml`  
`│   ├── tauri.conf.json`  
`│   └── src/`  
`│       ├── main.rs`  
`│       ├── audio/`  
`│       │   ├── mod.rs             # Inicialización de cpal y stream WASAPI`  
`│       │   ├── engine.rs          # Loop de render y mezcla de voces`  
`│       │   ├── sample.rs          # Estructuras PCM en RAM`  
`│       │   └── voice.rs           # Estado de voz activa (reproducción y ganancia)`  
`│       ├── midi/`  
`│       │   ├── mod.rs             # Gestión de conexión con midir`  
`│       │   ├── sniffer.rs         # Modo spike de calibración / captura raw`  
`│       │   └── parser.rs          # Decodificador de Note On/Off y CC`  
`│       ├── mapping/`  
`│       │   ├── mod.rs             # Matriz de mapeo: ControlID -> AudioAction`  
`│       │   └── store.rs           # Persistencia de mappings (JSON)`  
`│       └── state.rs               # Estado global compartido (hilo principal / IPC)`  
`├── src/                           # Frontend React`  
`│   ├── components/`  
`│   │   ├── ControllerView/        # Render del teclado, 8 pads, knobs y faders`  
`│   │   ├── MappingMatrix/         # Panel de asignación de archivos .wav`  
`│   │   └── AudioSettings/         # Selector de salidas de audio y buffers`  
`│   ├── hooks/`  
`│   │   ├── useMidiStream.ts       # Escucha de eventos Tauri en tiempo real`  
`│   │   └── useAudioConfig.ts      # Consulta y selección de dispositivos`  
`│   ├── types/`  
`│   │   └── midi.ts                # Interfaces TypeScript sincronizadas con Rust`  
`│   ├── App.tsx`  
`│   └── main.tsx`  
`└── package.json`

## **4\. Contratos, Modelos y Protocolos**

### **Esquema de Eventos MIDI (Rust)**

`#[derive(Debug, Clone, Serialize, Deserialize)]`  
`pub enum MidiEventType {`  
    `NoteOn { note: u8, velocity: u8 },`  
    `NoteOff { note: u8 },`  
    `ControlChange { controller: u8, value: u8 },`  
    `PitchBend { value: i16 },`  
`}`

`#[derive(Debug, Clone, Serialize, Deserialize)]`  
`pub struct NormalizedMidiEvent {`  
    `pub channel: u8,`  
    `pub event: MidiEventType,`  
    `pub timestamp_ms: u64,`  
`}`

### **Contrato de Mapeo del Controlador**

`#[derive(Debug, Clone, Serialize, Deserialize)]`  
`pub enum ControlTarget {`  
    `TriggerSample { sample_id: String },`  
    `SetMasterVolume,`  
    `SetVoiceVolume { voice_group: u8 },`  
    `SetFilterCutoff { voice_group: u8 },`  
`}`

`#[derive(Debug, Clone, Serialize, Deserialize)]`  
`pub struct MappingEntry {`  
    `pub control_id: String,     // Ej: "PAD_1", "FADER_1", "KNOB_3", "KEY_60"`  
    `pub midi_trigger: MidiTriggerMatcher, // Coincidencia por CC # o Nota #`  
    `pub target_action: ControlTarget,`  
`}`

### **Contrato de Mensajería Inter-Thread (Lock-Free Command a Audio Engine)**

`pub enum AudioEngineCommand {`  
    `TriggerVoice {`  
        `sample_id: u32,`  
        `velocity: f32, // Normalizado 0.0 .. 1.0`  
    `},`  
    `StopVoice {`  
        `sample_id: u32,`  
    `},`  
    `SetMasterGain(f32),`  
    `LoadSample {`  
        `id: u32,`  
        `channels: u16,`  
        `sample_rate: u32,`  
        `data: Vec<f32>, // Cargado en hilo IO, pasado a ownership del engine`  
    `},`  
`}`

## **5\. Criterios de Aceptación del Sistema**

> 1. **Detección de Hardware:** El sniffer CLI/interno detecta y lista la interfaz Impact LX49+ y reporta mensajes válidos en consola al presionar teclas, pads y mover faders/knobs.  
> 2. **Latencia y Continuidad:** Al presionar un Pad o Tecla, el disparo del sample ocurre en ≤ 8 ms sin clicks, popping ni cuelgues del hilo de audio durante 10 minutos continuos de uso.  
> 3. **Mapeo Dinámico:** Asignar un .wav arrastrándolo a la interfaz sobre el "Pad 1" produce el sonido correspondiente de forma inmediata al golpear físicamente el Pad 1\.  
> 4. **Enrutamiento de Salida:** La UI lista todos los dispositivos de salida WASAPI disponibles en Windows y permite conmutar entre ellos sin reiniciar la aplicación.  
> 5. **Estabilidad:** La desconexión o reconexión en caliente del cable USB del controlador no genera un pánico (panic) en la aplicación; el sistema pasa a estado desconectado y permite reconectar el puerto.