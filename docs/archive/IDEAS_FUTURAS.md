# Banco de Ideas Futuras e Innovaciones - ImpactSampler Studio

> Documento de captura conceptual, análisis de especificaciones y evaluación de viabilidad para nuevas funcionalidades antes de su formalización en el [PLAN_DE_TRABAJO.md](file:///C:/Users/WLADI/Documents/ImpactSamplerStudio/PLAN_DE_TRABAJO.md).
> 
> *Organizado en orden ascendente de complejidad y esfuerzo técnico de implementación (de menor a mayor dificultad).*

---

## 📌 Resumen de Objetivos por Nivel de Dificultad

| # | Objetivo | Nivel de Dificultad | Área Principal |
|:---:|---|:---:|---|
| **1** | **Teclado de Computadora (Entrada QWERTY)** | 🟢 Baja | Frontend (React/TS) + MIDI Bus |
| **2** | **Personalización del Instalador Tradicional & Branding Visual** | 🟢 Baja - Media | Activos Gráficos + Tauri Bundler (NSIS) |
| **3** | **Motor de Arpegiador Melódico Integrado** | 🟡 Media | Audio DSP Lock-Free (Rust) + UI Popover |
| **4** | **Sistema de Auto-Actualizaciones OTA (Tauri Updater)** | 🟡 Media - Alta | DevOps (GitHub Releases) + Tauri Updater |
| **5** | **Drum Kit Creator (WAV Pool a SF2 Compiler)** | 🔴 Alta | Codificador Binario RIFF SF2 (Rust) + UI Wizard |
| **6** | **Estrategia Multi-Plataforma (Mobile, Web App & VST3/CLAP)** | ⚫ Muy Alta (Estratégica) | Proyectos Derivados & Toolchains Externos |

---

---

## 1. 💻 Teclado de Computadora (Entrada QWERTY)

### Definición Funcional y Comportamiento Aprobado
- **Activación & Control UI:**
  - Botón toggle en el **Header** con icono minimalista de teclado (`QWERTY [ON/OFF]`) y LED de estado.
  - **Desactivado por defecto** al abrir la app para no interferir con atajos globales (como la barra espaciadora para Play/Pause o combinaciones del sistema).
  - Al activarse, captura los eventos del teclado físico y desactiva el auto-repeat (`event.repeat === false`) para evitar re-triggers ametrallados no deseados.
- **Disposición de Teclas (Estándar DAW):**
  - **Notas Blancas (Fila Media):** `A` (C), `S` (D), `D` (E), `F` (F), `G` (G), `H` (A), `J` (B), `K` (C+1), `L` (D+1).
  - **Notas Negras / Sostenidos (Fila Superior):** `W` (C#), `E` (D#), `T` (F#), `Y` (G#), `U` (A#), `O` (C#+1), `P` (D#+1).
  - **Cambio de Octava:** `Z` (bajar 1 octava) / `X` (subir 1 octava). Rango audible C1 a C6 con indicador de octava activa en pantalla.
  - **Ajuste de Velocidad:** `C` (-10 velocidad) / `V` (+10 velocidad), con velocidad nominal predeterminada de 100 o 127.
- **Feedback Visual & Guía Tipográfica:**
  - **Animación física 1:1:** Al pulsar una tecla de la computadora, la tecla correspondiente del piano virtual en pantalla se hunde y se ilumina reactivamente de manera indistinguible a una pulsación por hardware MIDI.
  - **Serigrafía Guía Dinámica:** Mientras el modo QWERTY está activo, sobre las teclas del piano virtual se proyectan de forma sutil y elegante las letras correspondientes (`A`, `W`, `S`, `E`...) para orientar al intérprete visualmente en tiempo real.

### Arquitectura de Despacho
- Hook de escucha `useQwertyKeyboard` en React que traduce `keydown` / `keyup` directamente a llamadas lock-free a Rust (`send_midi_note_on` / `send_midi_note_off`).
- Bypass automático cuando un input de texto, modal o diálogo de renombrado tiene el foco del cursor.

---

## 2. 📦 Personalización del Instalador Tradicional & Branding Visual

### Definición Funcional y Componentes Visuales
- **Branding e Iconografía Oficial:**
  - Creación de la identidad visual Noir-Tech oficial de **ImpactSampler Studio** (ej. isotipo con pad retroiluminado esmeralda/rojo o monograma de estudio de audio).
  - Generación del paquete completo de iconos mediante el toolchain de Tauri (`npm run tauri icon`):
    - Archivo `.ico` multi-capa para Windows (16x16, 32x32, 48x48, 64x64, 128x128, 256x256).
    - Icono de ventana, barra de tareas y explorador de archivos.
- **Personalización Visual del Instalador Windows (NSIS Bundle):**
  - **Formato exclusivo:** Instalador tradicional ejecutable (`.exe`), descartando versión portable para asegurar registro adecuado de extensiones y accesos directos.
  - **Gráficos Integrados del Asistente:**
    - **`installerSidebar` (164x314 px):** Arte gráfico vertical Noir-Tech de bienvenida con el logotipo, atmósfera de luces de estudio y textura acústica.
    - **`installerHeader` (150x57 px):** Banner horizontal minimalista para las páginas intermedias del instalador.
  - **Configuración Regional y Ergonomía:**
    - Asistente configurado 100% en español.
    - Opciones para crear acceso directo en el Escritorio y en el Menú Inicio.
    - Definición del directorio de instalación oficial (`%LOCALAPPDATA%\Programs\ImpactSamplerStudio` o `%PROGRAMFILES%`).

### Configuración en `tauri.conf.json`
- Inclusión formal de la sección `bundle.windows.nsis`:
  ```json
  "nsis": {
    "installerIcon": "icons/icon.ico",
    "headerImage": "installer/header.bmp",
    "sidebarImage": "installer/sidebar.bmp",
    "languages": ["Spanish"],
    "displayLanguageSelector": false
  }
  ```

---

## 3. 🎹 Motor de Arpegiador Melódico Integrado

### Definición Funcional y Comportamiento Aprobado
- **Activación & Control UI:**
  - Botón dedicado de activación rápida en la consola (`ARP [ON/OFF]`) con indicador LED reactivo.
  - Al interactuar, despliega un **popover / pestaña minimalista flotante** con los ajustes detallados del arpegio.
  - **Ubicación ergonómica:** Posicionado inmediatamente debajo del módulo de **FX ROUTE** (aprovechando una optimización de márgenes verticales en esa zona para mantener cero scroll y chasis compacto).
- **Sincronización:**
  - Opera de forma autónoma: al estar encendido (`ARP ON`), basta con presionar teclas para que el arpegio arranque y se sincronice dinámicamente con el tempo en BPM (y si el secuenciador corre, permanece en fase sample-accurate).
- **Función Latch / Hold (Comportamiento Clásico):**
  - Botón `HOLD / LATCH`: al soltar las teclas físicas, el arpegio sigue reproduciéndose en bucle infinito al tempo actual.
  - **Re-trigger Inteligente:** Al tocar un nuevo conjunto de notas, reemplaza de inmediato el buffer anterior sin cortes abruptos.

### Modos de Patrón y Benchmark de Estándares de la Industria
Para una experiencia completa y profesional desde el día uno, adoptaremos los modos canónicos de sintetizadores legendarios (Roland Juno/Jupiter, Korg MicroKorg, Arturia):

1. **`UP`:** De la nota más grave a la más aguda.
2. **`DOWN`:** De la nota más aguda a la más grave.
3. **`UP / DOWN` (Exclusivo - Estándar):** Sube y baja sin repetir las notas extremas (ejemplo con Do-Mi-Sol: Do → Mi → Sol → Mi → Do...).
4. **`UP / DOWN` (Inclusivo):** Sube y baja repitiendo las notas extremas (Do → Mi → Sol → Sol → Mi → Do...).
5. **`AS PLAYED` (Orden de pulsación):** Muy valorado en producción moderna para crear arpegios con ritmos melódicos personalizados según el orden en que los dedos tocan el teclado.
6. **`RANDOM`:** Selección aleatoria no repetitiva de las notas sostenidas.
7. **`CHORD` (Gater / Poly-Step):** Dispara todas las notas del acorde al unísono al ritmo de la cuadrícula seleccionada.

### Parámetros del Popover de Ajustes
- **Rate (División temporal):** `1/4`, `1/8`, `1/8T` (tresillos), `1/16`, `1/16T`, `1/32`.
- **Octave Range:** `1`, `2`, `3` o `4` octavas (con transposición sucesiva por ciclo).
- **Gate Length:** `20%` a `100%` (controla si las notas son staccato percusivo o legato sostenido).
- **Swing:** `0%` a `75%` para groove orgánico.

### Arquitectura en Rust (Audio Engine)
- Buffer lock-free de notas activas (`held_notes: [u8; 16]`).
- Integrado en el ciclo `render_internal` contando muestras con el reloj maestro, inyectando Note-On / Note-Off a `rustysynth` sin retraso de audio ni alocaciones en el hilo de audio.

---

## 4. 🔄 Sistema de Auto-Actualizaciones OTA (Tauri Updater)

### Definición Funcional y Comportamiento Aprobado
- **Infraestructura de Distribución:**
  - Alojamiento oficial mediante **GitHub Releases**.
  - Binarios firmados criptográficamente con par de claves privada/pública generadas por Tauri CLI (`tauri signer`).
  - Endpoint de comprobación apuntando al manifiesto `latest.json` servido directamente por la API de GitHub Releases.
- **Experiencia de Usuario (Flujo Híbrido No Intrusivo):**
  - **Comprobación Silenciosa al Inicio (Opción A):** Al abrir la aplicación, un hilo en segundo plano verifica si hay una versión superior a la actual. Si existe:
    - Enciende un indicador discreto en el **Header** (halo esmeralda o badge sutil con icono de descarga).
    - Cero interrupciones de la sesión de interpretación ni banners invasivos.
    - Al hacer clic en el badge, se despliega un popover informativo con la versión disponible, notas de la versión (*changelog*) y botón *«Descargar y Reiniciar»*.
  - **Comprobación Manual desde Ajustes (Opción C):**
    - En `AudioSettingsModal.tsx`, sección *«Acerca de & Actualizaciones»* con el número de versión activa (ej. `v1.0.0`) y botón interactivo *«Buscar Actualizaciones»* con feedback de estado (animación de carga, *«Tu sistema está actualizado»* o botón de descarga directa).
- **Preservación Estricta de Datos del Usuario:**
  - Garantía absoluta de persistencia: la actualización reemplaza únicamente el binario de la aplicación (`ImpactSamplerStudio.exe`) y sus librerías de fábrica (`resources/`).
  - **Datos Blindados:**
    - Directorio de librerías y SoundFonts personalizados del usuario (`User Library`).
    - Archivos de proyectos y sesiones musicales (`.impact`).
    - Configuraciones en `localStorage` (calibración de latencia de 8 golpes, mapeos de MIDI Learn de los 17 controles y perfiles de hardware).

### Configuración en `tauri.conf.json`
- Integración del plugin oficial `@tauri-apps/plugin-updater`:
  ```json
  "plugins": {
    "updater": {
      "pubkey": "...",
      "endpoints": [
        "https://github.com/<usuario>/ImpactSamplerStudio/releases/latest/download/latest.json"
      ]
    }
  }
  ```

---

## 5. 🥁 Drum Kit Creator (WAV Pool a SF2 Compiler)

### Definición Funcional y Flujo de Trabajo Aprobado
- **Concepto Central:**
  Permite al usuario crear sus propios Drum Kits SoundFont (`.sf2`) a partir de una colección indeterminada de archivos `.wav` (desde 8 hasta decenas de muestras de percusión, como el set completo del Yamaha RX7), compilando un binario `.sf2` estándar compatible con el motor `rustysynth` y con DAWs externos.
- **Flujo de Creación (Wizard Noir-Tech):**
  1. **Carga Masiva de Muestras (Sample Pool):**
     - El usuario arrastra una carpeta o selección múltiple de archivos `.wav` al modal de creación.
     - La app lista todas las muestras cargadas con botón de audición rápida (*preview* instantáneo).
  2. **Auto-Asignación Inteligente Inicial a los 8 Pads:**
     - El sistema analiza los nombres de archivo y asigna automáticamente las 8 posiciones principales a los pads físicos por heurística tímbrica (Kick -> P1, Snare -> P2, Clap -> P3, Closed HH -> P4, Open HH -> P5, Low Tom -> P6, High Tom -> P7, Crash/Perc -> P8).
  3. **Reasignación Dinámica por Clic Derecho:**
     - En cualquier momento durante la sesión, al hacer **clic derecho** sobre cualquiera de los 8 slots de pads y elegir *«Asignar Muestra del SoundFont»*, se despliega un menú contextual con **la lista completa de todos los sonidos contenidos en el kit** para intercambiarlos al vuelo.
  4. **Compilación a SoundFont 2 (`.sf2`):**
     - El usuario asigna un nombre al kit (ej. *«TR-808 Vintage Collection»*).
     - Al presionar *«Crear y Compilar Kit (.sf2)»*, el motor en Rust ensambla la estructura binaria RIFF SF2, lo guarda en el directorio de usuario (`User Library`) y lo selecciona inmediatamente como instrumento activo sin reiniciar la app.

### Mapeo en el Teclado y Pads
- Cada muestra del kit se ubica en una nota MIDI consecutiva a lo largo de las teclas (ej. a partir de C1 / Nota 36 hacia arriba), permitiendo que:
  - Los 8 pads físicos disparen sus 8 sonidos seleccionados.
  - El teclado completo de 49 teclas permita tocar la totalidad de las muestras de la colección cromáticamente.

### Arquitectura Técnica en Rust
- **Módulo `sf2_builder` en Rust:**
  - Decodificación y normalización de los WAVs con `hound`.
  - Construcción de los chunks estándar del formato SoundFont 2.04:
    - Chunk `INFO` (nombre del banco, autor, software).
    - Chunk `sdta` (datos PCM de 16-bit intercalados en el sample buffer).
    - Chunk `pdta` (definición de generadores: sample headers, root key, zones, instrumentos y preset Bank 128 / Preset 0).
  - Tarea en segundo plano con barra de progreso en la UI para ensamblados grandes.

---

## 6. 🌐 Estrategia Multi-Plataforma (Mobile, Web App & VST3/CLAP)

### Definición Funcional y Prioridades Estratégicas
Se establece una hoja de ruta ordenada por impacto de usuario y reutilización del stack tecnológico:

1. **Prioridad 1: Aplicación Móvil (iOS / iPadOS & Android):**
   - **Objetivo:** Llevar ImpactSampler Studio a tablets y smartphones aprovechando las capacidades nativas de compilación multiplataforma de Tauri v2 y Rust.
   - **Enfoque técnico:**
     - En el frontend: diseño *touch-first* con gestos táctiles (multitouch para tocar acordes en pantalla o pads simultáneos).
     - En el backend: sustitución del backend WASAPI por **Oboe / AAudio** en Android y **CoreAudio** en iOS/iPadOS, manteniendo el motor `rustysynth` y sampler intactos.
     - Soporte para controladores MIDI USB-C / Lightning (CoreMIDI en iOS, Android MIDI API).

2. **Prioridad 2: Web App (Browser Standalone):**
   - **Objetivo:** Versión accesible desde cualquier navegador moderno sin requerir instalación previa.
   - **Enfoque técnico:**
     - Compilación de la lógica DSP a **WebAssembly (WASM)**.
     - Audio mediante **Web Audio API (`AudioWorkletNode`)** para garantizar procesamiento de audio en hilo separado.
     - Conexión de hardware MIDI mediante **Web MIDI API** nativa de navegadores Chromium.

3. **Prioridad 3: Plugin VST3 / CLAP (Integración en DAWs):**
   - **Objetivo:** Abrir ImpactSampler Studio como instrumento virtual dentro de Reaper, Ableton Live, FL Studio, Cubase, etc.
   - **Interfaz:** Misma estética visual Noir-Tech completa y monolítica, pero integrada en la ventana de host VST3 sin bordes de ventana del sistema operativo.
   - **Sincronización Bidireccional:**
     - Transporte y BPM esclavizados automáticamente al compás del DAW (Play, Stop, Tempo changes y compás).
     - Automatización de parámetros: exponer los 8 knobs (Cutoff, Reverb, etc.) y faders a la línea de tiempo de automatización del DAW.

### Estrategia de Arquitectura & Blindaje del Proyecto Desktop
- **Aislamiento Total por Proyectos Separados:**
  - Para blindar la estabilidad del proyecto actual de escritorio (Desktop Windows) y evitar regresiones o dependencias complejas, cada formato se concebirá como **un proyecto/repositorio derivado independiente** o workspace modular.
  - El proyecto Desktop continuará con su configuración hiper-optimizada para Windows WASAPI de baja latencia sin contaminarse con condicionales multiplataforma innecesarios.
