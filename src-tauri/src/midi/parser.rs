use serde::{Deserialize, Serialize};

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

pub fn parse_midi_message(port_name: &str, bytes: &[u8]) -> Option<NormalizedMidiEvent> {
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
            let pad_index = get_pad_index(note, channel);

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
            let pad_index = get_pad_index(note, channel);

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

pub fn get_pad_index(note: u8, channel: u8) -> Option<u8> {
    // Solo los mensajes del Canal 10 (canal estándar de pads de batería del Nektar Impact LX49+) activan pads.
    // Esto garantiza que el teclado melódico (Canal 1 u otros) toque las notas graves 36..43 sin secuestro.
    // Los pads físicos van estrictamente consecutivos: PAD1 (36) -> PAD8 (43).
    if channel == 10 {
        match note {
            36..=43 => Some(note - 35),
            _ => None,
        }
    } else {
        None
    }
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

        // Botones Asignables Nektar LX49+ (B1..B9)
        47 => Some("Button 1 (Click On/Off)".to_string()),
        48 => Some("Button 2 (Grid 1/8)".to_string()),
        49 => Some("Button 3 (Grid 1/16)".to_string()),
        50 => Some("Button 4 (Grid 1/32)".to_string()),
        51 => Some("Button 5 (Pattern Bank A/B)".to_string()),
        52 => Some("Button 6 (MPC Swing 58%)".to_string()),
        53 => Some("Button 7 (Clear Track)".to_string()),
        54 => Some("Button 8 (Toggle Visual Metronome)".to_string()),
        55 => Some("Button 9 (Clear All Pattern)".to_string()),

        _ => None,
    }
}
