pub mod parser;

use std::sync::{Arc, Mutex};
use midir::{Ignore, MidiInput, MidiInputConnection};
use rtrb::Producer;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::audio::{AudioEngineCommand, StepResolution};
use parser::{parse_midi_message, MidiEventType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiPortInfo {
    pub index: usize,
    pub name: String,
    pub is_default: bool,
    pub is_connected: bool,
}

pub struct MidiManager {
    _connections: Vec<MidiInputConnection<()>>,
    pub connected_port_index: Option<usize>,
    pub connected_port_name: Option<String>,
}

impl MidiManager {
    pub fn new() -> Self {
        Self {
            _connections: Vec::new(),
            connected_port_index: None,
            connected_port_name: None,
        }
    }

    pub fn list_ports(&self) -> Vec<MidiPortInfo> {
        let mut result = Vec::new();
        if let Ok(midi_in) = MidiInput::new("ImpactSampler Probe") {
            let ports = midi_in.ports();
            for (idx, p) in ports.iter().enumerate() {
                if let Ok(name) = midi_in.port_name(p) {
                    let is_default = (name.to_lowercase().contains("impact")
                        || name.to_lowercase().contains("lx49")
                        || name.to_lowercase().contains("nektar"))
                        && !name.to_lowercase().contains("midiin2")
                        && !name.to_lowercase().contains("daw");
                    let is_connected = self.connected_port_index == Some(idx);
                    result.push(MidiPortInfo {
                        index: idx,
                        name,
                        is_default,
                        is_connected,
                    });
                }
            }
        }
        result
    }

    pub fn connect_to_port(
        &mut self,
        target_idx: usize,
        app_handle: AppHandle,
        audio_producer: Arc<Mutex<Producer<AudioEngineCommand>>>,
    ) -> Result<Vec<MidiPortInfo>, String> {
        self._connections.clear();
        self.connected_port_index = None;
        self.connected_port_name = None;

        let midi_scan = MidiInput::new("ImpactSampler Scanner")
            .map_err(|e| format!("Error al inicializar MIDI: {}", e))?;

        let in_ports = midi_scan.ports();
        if target_idx >= in_ports.len() {
            return Err(format!("Índice de puerto {} fuera de rango", target_idx));
        }

        let port = &in_ports[target_idx];
        let port_name = midi_scan
            .port_name(port)
            .unwrap_or_else(|_| format!("Port-{}", target_idx));

        let mut midi_in = MidiInput::new("ImpactSampler CoreIn")
            .map_err(|e| format!("Error creando cliente MIDI: {}", e))?;
        midi_in.ignore(Ignore::None);

        let app_clone = app_handle.clone();
        let prod_clone = audio_producer.clone();
        let p_name = port_name.clone();

        let conn = midi_in.connect(
            port,
            "impact-primary-input",
            move |_stamp, bytes, _| {
                if let Some(event) = parse_midi_message(&p_name, bytes) {
                    // 1. Emitir evento a la UI de Tauri en tiempo real
                    let _ = app_clone.emit("midi-event", &event);

                    // 2. Despachar comando lock-free al motor de audio
                    if let Ok(mut prod) = prod_clone.lock() {
                        match &event.event {
                            MidiEventType::NoteOn { note, velocity, pad_index, .. } => {
                                let vel_f = *velocity as f32 / 127.0;
                                if let Some(p_idx) = pad_index {
                                    let _ = prod.push(AudioEngineCommand::PadTrigger {
                                        pad_index: (*p_idx as usize).saturating_sub(1),
                                        velocity: vel_f,
                                    });
                                } else {
                                    let _ = prod.push(AudioEngineCommand::NoteOn {
                                        note: *note,
                                        velocity: vel_f,
                                    });
                                }
                            }
                            MidiEventType::NoteOff { note, pad_index, .. } => {
                                if let Some(p_idx) = pad_index {
                                    let _ = prod.push(AudioEngineCommand::PadRelease {
                                        pad_index: (*p_idx as usize).saturating_sub(1),
                                    });
                                } else {
                                    let _ = prod.push(AudioEngineCommand::NoteOff { note: *note });
                                }
                            }
                            MidiEventType::ControlChange { controller, value, .. } => {
                                let val_f = *value as f32 / 127.0;
                                let _ = prod.push(AudioEngineCommand::ControlChange {
                                    controller: *controller,
                                    value: val_f,
                                });

                                // Interceptar botones físicos de Transporte y Asignables del Nektar Impact LX49+
                                if *value > 0 {
                                    match *controller {
                                        102 => {
                                            // Cycle / Loop: Tap Tempo
                                            let _ = prod.push(AudioEngineCommand::SequencerTapTempo);
                                        }
                                        103 => {
                                            // Rewind: BPM -1
                                            let _ = prod.push(AudioEngineCommand::SequencerAdjustBpm(-1.0));
                                        }
                                        104 => {
                                            // Forward: BPM +1
                                            let _ = prod.push(AudioEngineCommand::SequencerAdjustBpm(1.0));
                                        }
                                        105 => {
                                            // Stop: Detener y rebobinar
                                            let _ = prod.push(AudioEngineCommand::SequencerStop);
                                        }
                                        106 => {
                                            // Play: Iniciar / Pausar
                                            let _ = prod.push(AudioEngineCommand::SequencerTogglePlay);
                                        }
                                        107 => {
                                            // Record: Alternar modo Overdub en vivo
                                            let _ = prod.push(AudioEngineCommand::SequencerToggleRecord);
                                        }
                                        // Botones Asignables B1..B9 (CC 47..55)
                                        47 => {
                                            // B1: Metrónomo Sonoro Clic ON/OFF
                                            let _ = prod.push(AudioEngineCommand::SequencerToggleClick);
                                        }
                                        48 => {
                                            // B2: Grid 1/8
                                            let _ = prod.push(AudioEngineCommand::SequencerSetResolution(StepResolution::Eighth));
                                        }
                                        49 => {
                                            // B3: Grid 1/16
                                            let _ = prod.push(AudioEngineCommand::SequencerSetResolution(StepResolution::Sixteenth));
                                        }
                                        50 => {
                                            // B4: Grid 1/32
                                            let _ = prod.push(AudioEngineCommand::SequencerSetResolution(StepResolution::ThirtySecond));
                                        }
                                        51 => {
                                            // B5: Pattern Bank A/B
                                            let _ = prod.push(AudioEngineCommand::SequencerToggleBank);
                                        }
                                        52 => {
                                            // B6: Swing MPC 58% ON/OFF
                                            let _ = prod.push(AudioEngineCommand::SequencerToggleSwing);
                                        }
                                        53 => {
                                            // B7: Clear Track (limpia la pista del pad seleccionado)
                                            let _ = prod.push(AudioEngineCommand::SequencerClearSelectedTrack);
                                        }
                                        54 => {
                                            // B8: Metrónomo Visual Gigante ON/OFF
                                            let _ = app_clone.emit("toggle-visual-metronome", ());
                                        }
                                        55 => {
                                            // B9: Clear All Pattern (limpia la grilla del banco activo)
                                            let _ = prod.push(AudioEngineCommand::SequencerClear);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            MidiEventType::PitchBend { normalized, .. } => {
                                let _ = prod.push(AudioEngineCommand::PitchBend {
                                    normalized: *normalized,
                                });
                            }
                        }
                    }
                }
            },
            (),
        ).map_err(|e| format!("Error al conectar al puerto MIDI '{}': {}", port_name, e))?;

        println!("[MIDI OK] Conectado exitosamente al puerto [{}]: '{}'", target_idx, port_name);
        self._connections.push(conn);
        self.connected_port_index = Some(target_idx);
        self.connected_port_name = Some(port_name);

        Ok(self.list_ports())
    }

    pub fn start_auto_connect(
        &mut self,
        app_handle: AppHandle,
        audio_producer: Arc<Mutex<Producer<AudioEngineCommand>>>,
    ) -> Result<Vec<MidiPortInfo>, String> {
        let midi_scan = MidiInput::new("ImpactSampler ProbeScan")
            .map_err(|e| format!("Error al inicializar MIDI input: {}", e))?;

        let in_ports = midi_scan.ports();
        if in_ports.is_empty() {
            return Err("No se encontraron puertos MIDI de entrada en el sistema".to_string());
        }

        // Seleccionar preferentemente el puerto Impact LX49+ que no sea MIDIIN2 ni DAW
        let mut target_idx = 0;
        let mut found_preferred = false;

        for (idx, port) in in_ports.iter().enumerate() {
            if let Ok(name) = midi_scan.port_name(port) {
                let name_lower = name.to_lowercase();
                let is_impact = name_lower.contains("impact")
                    || name_lower.contains("lx49")
                    || name_lower.contains("nektar");
                let is_secondary = name_lower.contains("midiin2")
                    || name_lower.contains("daw")
                    || name_lower.contains("ctrl");

                if is_impact && !is_secondary {
                    target_idx = idx;
                    found_preferred = true;
                    break;
                }
            }
        }

        if !found_preferred {
            for (idx, port) in in_ports.iter().enumerate() {
                if let Ok(name) = midi_scan.port_name(port) {
                    if !name.to_lowercase().contains("midiin2") {
                        target_idx = idx;
                        break;
                    }
                }
            }
        }

        self.connect_to_port(target_idx, app_handle, audio_producer)
    }
}
