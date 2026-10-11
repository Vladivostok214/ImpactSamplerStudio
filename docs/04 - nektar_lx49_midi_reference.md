# Nektar Impact LX49+ - Referencia Técnica MIDI y Layout

> Documento de referencia rápida extraído del manual de usuario y especificaciones de fábrica para **ImpactSampler Studio**.

---

## 1. User Preset 1: GM Instrument (Mapeo MIDI por Defecto)

Canal MIDI: **Global (Omni / Ch 1 por defecto)**.

### 1.1 Faders (F1 - F9)

| Control | Mensaje MIDI | CC # | Rango | Función Hardware / Fábrica | Destino Sugerido en Sampler |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **F1** | MIDI CC | **73** | 0 - 127 | Global Attack | Envolvente Attack |
| **F2** | MIDI CC | **75** | 0 - 127 | Global Decay | Envolvente Decay |
| **F3** | MIDI CC | **72** | 0 - 127 | Global Release | Envolvente Release |
| **F4** | MIDI CC | **91** | 0 - 127 | Effect Depth 1 (Reverb Send) | FX / Aux Send 1 |
| **F5** | MIDI CC | **92** | 0 - 127 | Effect Depth 2 | FX / Aux Send 2 |
| **F6** | MIDI CC | **93** | 0 - 127 | Effect Depth 3 (Chorus Send) | FX / Aux Send 3 |
| **F7** | MIDI CC | **94** | 0 - 127 | Effect Depth 4 | FX / Aux Send 4 |
| **F8** | MIDI CC | **95** | 0 - 127 | Effect Depth 5 | FX / Aux Send 5 |
| **F9** | MIDI CC | **7** | 0 - 127 | **Global Volume** | **Master Volume / Gain** |

### 1.2 Botones bajo Faders (B1 - B9)
Modo de fábrica: **MIDI CC (Toggle: 0 / 127)**

| Botón | Mensaje MIDI | CC # | Función Fábrica | Uso Potencial Sampler |
| :--- | :--- | :--- | :--- | :--- |
| **B1** | MIDI CC Toggle | **0** | Bank MSB | Mute Grupo / Muestra 1 |
| **B2** | MIDI CC Toggle | **2** | Breath | Mute Grupo / Muestra 2 |
| **B3** | MIDI CC Toggle | **3** | Control Change (Undefined) | Mute Grupo / Muestra 3 |
| **B4** | MIDI CC Toggle | **4** | Foot Controller | Mute Grupo / Muestra 4 |
| **B5** | MIDI CC Toggle | **6** | Data Entry MSB | Mute Grupo / Muestra 5 |
| **B6** | MIDI CC Toggle | **8** | Balance | Mute Grupo / Muestra 6 |
| **B7** | MIDI CC Toggle | **9** | Control Change (Undefined) | Mute Grupo / Muestra 7 |
| **B8** | MIDI CC Toggle | **11** | Expression Controller | Mute Grupo / Muestra 8 |
| **B9** | MIDI CC Toggle | **65** | Portamento On/Off *(Global)* | Solo / Mute Master / Bypass |

### 1.3 Knobs / Potenciómetros (K1 - K8)

| Knob | Mensaje MIDI | CC # | Rango | Función Fábrica | Destino Sugerido Sampler |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **K1** | MIDI CC | **74** | 0 - 127 | Brightness | **Filter Cutoff (LPF)** |
| **K2** | MIDI CC | **71** | 0 - 127 | Harmonic Content | **Filter Resonance (Q)** |
| **K3** | MIDI CC | **5** | 0 - 127 | Portamento Rate | Pitch / Glide |
| **K4** | MIDI CC | **84** | 0 - 127 | Portamento Depth | Pitch Modulation |
| **K5** | MIDI CC | **78** | 0 - 127 | Vibrato Delay | LFO Delay |
| **K6** | MIDI CC | **76** | 0 - 127 | Vibrato Rate | LFO Rate |
| **K7** | MIDI CC | **77** | 0 - 127 | Vibrato Depth | LFO Depth |
| **K8** | MIDI CC | **10** | 0 - 127 | Global Pan | **Master Stereo Pan** |

---

## 2. Configuración de Pads (Mapas de Fábrica)

El hardware almacena 4 mapas internos seleccionables desde el botón *Pad Maps / Clips / Scenes*.

### 2.1 Pad Map 1 (Default Cromático - Octava 1)

| Pad Físico | Nota MIDI | Número MIDI | Rango Velocity | Comportamiento |
| :--- | :--- | :--- | :--- | :--- |
| **Pad 1** | C1 | **36** | 0 - 127 | Note On / Off |
| **Pad 2** | C#1 | **37** | 0 - 127 | Note On / Off |
| **Pad 3** | D1 | **38** | 0 - 127 | Note On / Off |
| **Pad 4** | D#1 | **39** | 0 - 127 | Note On / Off |
| **Pad 5** | E1 | **40** | 0 - 127 | Note On / Off |
| **Pad 6** | F1 | **41** | 0 - 127 | Note On / Off |
| **Pad 7** | F#1 | **42** | 0 - 127 | Note On / Off |
| **Pad 8** | G1 | **43** | 0 - 127 | Note On / Off |

### 2.2 Pad Map 2 (Cromático - Octava 2)
* **P1:** G#1 (44) | **P2:** A1 (45) | **P3:** A#1 (46) | **P4:** B1 (47)
* **P5:** C2 (48) | **P6:** C#2 (49) | **P7:** D2 (50) | **P8:** D#2 (51)

### 2.3 Pad Map 3 (Cromático - Octava 3)
* **P1:** C3 (60) | **P2:** D3 (62) | **P3:** E3 (64) | **P4:** F3 (65)
* **P5:** G3 (67) | **P6:** A3 (69) | **P7:** B3 (71) | **P8:** C4 (72)

### 2.4 Pad Map 4 (Estándar GM Drum Kit)

| Pad Físico | Nota MIDI | Número MIDI | Asignación GM Estándar |
| :--- | :--- | :--- | :--- |
| **Pad 1** | C1 | **36** | Bass Drum (Kick) |
| **Pad 2** | D1 | **38** | Acoustic Snare |
| **Pad 3** | F#1 | **42** | Closed Hi-Hat |
| **Pad 4** | A#1 | **46** | Open Hi-Hat |
| **Pad 5** | G1 | **43** | Low Floor Tom |
| **Pad 6** | A1 | **45** | Low Tom |
| **Pad 7** | C#1 | **37** | Side Stick |
| **Pad 8** | C#2 | **49** | Crash Cymbal 1 |

---

## 3. Teclado y Ruedas de Expresión

* **Teclas (49 teclas sensibles a la velocidad):**
  * Rango por defecto: **C1 (36) a C5 (84)**.
  * Con botones Octave/Transpose: extensible desde C-2 (0) hasta G8 (127).
* **Ruedas Físicas:**
  * **Pitch Bend:** Mensaje de Pitch Wheel (`0` a `16383`, centro en `8192`).
  * **Modulation Wheel:** MIDI CC **1** (0 - 127).

---

## 4. Distribución Visual del Hardware (Para UI Frontend)

```
┌────────────────────────────────────────────────────────────────────────┐
│ [NEKTAR]                                          [DISPLAY] [K1-K4] [PADS 1-4] │
│ [PITCH]  [F1 F2 F3 F4 F5 F6 F7 F8 F9]             [ 0.62  ] [K5-K8] [PADS 5-8] │
│ [MOD]    [B1 B2 B3 B4 B5 B6 B7 B8 B9]             [TRANS.]  [TRNSP] [LEARN]    │
│ ┌────────────────────────────────────────────────────────────────────┐ │
│ │ || | ||| | || | ||| | || | ||| | || | ||| |                        │ │
│ │ || | ||| | || | ||| | || | ||| | || | ||| | (49 Teclas: C1 .. C5)  │ │
│ └────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```
