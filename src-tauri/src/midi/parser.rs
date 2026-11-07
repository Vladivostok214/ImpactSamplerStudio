use serde::{Deserialize, Serialize};

/// Mapa por defecto de notas MIDI de entrada para los 8 pads (orden estándar General MIDI / UI).
/// Pad 1: 36 (Kick), Pad 2: 38 (Snare), Pad 3: 42 (Closed Hat), Pad 4: 46 (Open Hat),
/// Pad 5: 39 (Clap), Pad 6: 45 (Low Tom), Pad 7: 48 (High Tom), Pad 8: 49 (Crash).
pub const DEFAULT_PAD_INPUT_NOTES: [u8; 8] = [36, 38, 42, 46, 39, 45, 48, 49];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MidiEventType {
    #[serde(rename_all = "camelCase")]
    NoteOn {
        note: u8,
        velocity: u8,
        note_name: String,
        pad_index: Option<u8>,
    },
    #[serde(rename_all = "camelCase")]
    NoteOff {
        note: u8,
        velocity: u8,
        note_name: String,
        pad_index: Option<u8>,
    },
    #[serde(rename_all = "camelCase")]
    ControlChange {
        controller: u8,
        value: u8,
        label: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    PitchBend {
        value: u16,
        normalized: i16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedMidiEvent {
    pub port_name: String,
    pub channel: u8,
    pub event: MidiEventType,
    pub timestamp_ms: u64,
}

#[allow(dead_code)]
pub fn parse_midi_message(port_name: &str, bytes: &[u8]) -> Option<NormalizedMidiEvent> {
    parse_midi_message_with_pad_map(port_name, bytes, &DEFAULT_PAD_INPUT_NOTES)
}

pub fn parse_midi_message_with_pad_map(
    port_name: &str,
    bytes: &[u8],
    pad_notes: &[u8; 8],
) -> Option<NormalizedMidiEvent> {
    if bytes.is_empty() {
        return None;
    }

    let status = bytes[0];
    let channel = (status & 0x0F) + 1;
    let msg_type = status & 0xF0;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    match msg_type {
        0x90 if bytes.len() >= 3 => {
            let note = bytes[1];
            let velocity = bytes[2];
            let note_name = get_note_name(note);
            let pad_index = resolve_pad_index(note, channel, pad_notes);

            if velocity == 0 {
                Some(NormalizedMidiEvent {
                    port_name: port_name.to_string(),
                    channel,
                    event: MidiEventType::NoteOff {
                        note,
                        velocity: 0,
                        note_name,
                        pad_index,
                    },
                    timestamp_ms: now_ms,
                })
            } else {
                Some(NormalizedMidiEvent {
                    port_name: port_name.to_string(),
                    channel,
                    event: MidiEventType::NoteOn {
                        note,
                        velocity,
                        note_name,
                        pad_index,
                    },
                    timestamp_ms: now_ms,
                })
            }
        }
        0x80 if bytes.len() >= 3 => {
            let note = bytes[1];
            let velocity = bytes[2];
            let note_name = get_note_name(note);
            let pad_index = resolve_pad_index(note, channel, pad_notes);

            Some(NormalizedMidiEvent {
                port_name: port_name.to_string(),
                channel,
                event: MidiEventType::NoteOff {
                    note,
                    velocity,
                    note_name,
                    pad_index,
                },
                timestamp_ms: now_ms,
            })
        }
        0xB0 if bytes.len() >= 3 => {
            let controller = bytes[1];
            let value = bytes[2];
            let label = get_cc_label(controller);

            Some(NormalizedMidiEvent {
                port_name: port_name.to_string(),
                channel,
                event: MidiEventType::ControlChange {
                    controller,
                    value,
                    label,
                },
                timestamp_ms: now_ms,
            })
        }
        0xE0 if bytes.len() >= 3 => {
            let lsb = bytes[1] as u16;
            let msb = bytes[2] as u16;
            let value = (msb << 7) | lsb;
            let normalized = value as i16 - 8192;

            Some(NormalizedMidiEvent {
                port_name: port_name.to_string(),
                channel,
                event: MidiEventType::PitchBend {
                    value,
                    normalized,
                },
                timestamp_ms: now_ms,
            })
        }
        _ => None,
    }
}

/// Resuelve el pad (`1..=8`) para una nota MIDI entrante siguiendo la política de prioridad:
///
/// 1. **Prioridad del mapeo activo de pads (`pad_notes`):** Se recorre `pad_notes` en orden
///    de índice `0..8` (`first-match wins`). Cualquier nota presente en `pad_notes` (ya sea del
///    mapa por defecto `[36, 38, 42, 46, 39, 45, 48, 49]` o aprendida vía MIDI Learn) resuelve
///    directamente a su pad (`1..=8`) sin depender del canal MIDI ni discriminar contra el Pad 1 (`36`).
/// 2. **Fallback General MIDI (exclusivo del Canal 10):** Si el mensaje entra por el Canal 10 y la
///    nota no coincide con ninguna entrada de `pad_notes`, se aplica el fallback General MIDI sólo
///    para aquellos pads que aún conservan su nota por defecto (evitando que la nota antigua de un
///    pad reasignado por MIDI Learn siga disparando ese pad por fallback).
/// 3. **Política ante asignaciones duplicadas:** La búsqueda evalúa los índices `0..8` en orden
///    ascendente y retorna inmediatamente el primer pad coincidente (`lowest pad index wins`),
///    garantizando que una misma nota nunca dispare dos pads.
pub fn resolve_pad_index(note: u8, channel: u8, pad_notes: &[u8; 8]) -> Option<u8> {
    // 1. Prioridad máxima: mapa activo de los 8 pads (por defecto o personalizados con MIDI Learn)
    for (idx, &mapped_note) in pad_notes.iter().enumerate() {
        if mapped_note == note {
            return Some((idx + 1) as u8);
        }
    }

    // 2. Canal 10: fallback General MIDI para notas alternativas de batería en pads no reasignados
    if channel == 10 {
        let fallback_pad = match note {
            36 => Some(1),           // Kick
            38 => Some(2),           // Snare
            42 => Some(3),           // Closed Hat
            46 => Some(4),           // Open Hat
            37 | 39 | 40 => Some(5), // Side Stick / Clap / Electric Snare
            41 | 43 | 45 => Some(6), // Low Floor / High Floor / Low Tom
            47 | 48 => Some(7),      // Low-Mid / Hi-Mid Tom
            49 | 51 => Some(8),      // Crash / Ride
            _ => None,
        };
        if let Some(pad_num) = fallback_pad {
            let idx = (pad_num - 1) as usize;
            // Solo aplicar fallback si ese pad no fue reasignado a otra nota personalizada
            if pad_notes[idx] == DEFAULT_PAD_INPUT_NOTES[idx] {
                return Some(pad_num);
            }
        }
    }

    None
}

#[allow(dead_code)]
pub fn get_pad_index(note: u8, channel: u8) -> Option<u8> {
    resolve_pad_index(note, channel, &DEFAULT_PAD_INPUT_NOTES)
}

pub fn get_note_name(note: u8) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (note as i32 / 12) - 1;
    let name = names[(note % 12) as usize];
    format!("{}{}", name, octave)
}

pub fn get_cc_label(cc: u8) -> Option<String> {
    match cc {
        // Bloque Correlativo Nektar LX49+ (Faders)
        38 => Some("Fader 1".to_string()),
        39 => Some("Fader 2".to_string()),
        40 => Some("Fader 3".to_string()),
        41 => Some("Fader 4".to_string()),
        42 => Some("Fader 5".to_string()),
        43 => Some("Fader 6".to_string()),
        44 => Some("Fader 7".to_string()),
        45 => Some("Fader 8".to_string()),
        46 => Some("Fader 9 (Master Volume)".to_string()),

        // Bloque Correlativo Nektar LX49+ (Knobs)
        56 => Some("Knob 1".to_string()),
        57 => Some("Knob 2".to_string()),
        58 => Some("Knob 3".to_string()),
        59 => Some("Knob 4".to_string()),
        60 => Some("Knob 5".to_string()),
        61 => Some("Knob 6".to_string()),
        62 => Some("Knob 7".to_string()),
        63 => Some("Knob 8".to_string()),

        // Preset Alternativo General MIDI
        73 => Some("Fader 1 (Attack)".to_string()),
        75 => Some("Fader 2 (Decay)".to_string()),
        72 => Some("Fader 3 (Release)".to_string()),
        91 => Some("Fader 4 (FX 1)".to_string()),
        92 => Some("Fader 5 (FX 2)".to_string()),
        93 => Some("Fader 6 (FX 3)".to_string()),
        94 => Some("Fader 7 (FX 4)".to_string()),
        95 => Some("Fader 8 (FX 5)".to_string()),
        7  => Some("Fader 9 (Master Volume)".to_string()),

        74 => Some("Knob 1 (Cutoff)".to_string()),
        71 => Some("Knob 2 (Resonance)".to_string()),
        5  => Some("Knob 3 (Portamento)".to_string()),
        84 => Some("Knob 4 (Port. Depth)".to_string()),
        78 => Some("Knob 5 (Vib Delay)".to_string()),
        76 => Some("Knob 6 (Vib Rate)".to_string()),
        77 => Some("Knob 7 (Vib Depth)".to_string()),
        10 => Some("Knob 8 (Global Pan)".to_string()),

        1  => Some("Modulation Wheel".to_string()),

        // Bloque de Transporte Nektar LX49+ (Fase 5)
        102 => Some("Transport: Cycle (Tap Tempo)".to_string()),
        103 => Some("Transport: Rewind (BPM -)".to_string()),
        104 => Some("Transport: Forward (BPM +)".to_string()),
        105 => Some("Transport: Stop".to_string()),
        106 => Some("Transport: Play / Pause".to_string()),
        107 => Some("Transport: Record (Overdub)".to_string()),

        // Botones Asignables Nektar LX49+ (B1..B9) - Libres para MIDI Learn en Paso 4.1
        47 => Some("Button 1 (Unassigned)".to_string()),
        48 => Some("Button 2 (Unassigned)".to_string()),
        49 => Some("Button 3 (Unassigned)".to_string()),
        50 => Some("Button 4 (Unassigned)".to_string()),
        51 => Some("Button 5 (Unassigned)".to_string()),
        52 => Some("Button 6 (Unassigned)".to_string()),
        53 => Some("Button 7 (Unassigned)".to_string()),
        54 => Some("Button 8 (Unassigned)".to_string()),
        55 => Some("Button 9 (Unassigned)".to_string()),

        _ => None,
    }
}
