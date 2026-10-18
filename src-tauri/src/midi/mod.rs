pub mod parser;

use std::sync::{Arc, Mutex};
use midir::{Ignore, MidiInput, MidiInputConnection};
use rtrb::Producer;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::audio::AudioEngineCommand;
use parser::{parse_midi_message, MidiEventType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiPortInfo {
    pub index: usize,
    pub name: String,
    pub is_default: bool,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiStatusEvent {
    pub connected: bool,
    pub port_name: Option<String>,
    pub ports: Vec<MidiPortInfo>,
}

use crate::audio::sequencer::SequencerAction;

pub struct MidiManager {
    _connections: Vec<MidiInputConnection<()>>,
    pub connected_port_index: Option<usize>,
    pub connected_port_name: Option<String>,
    pub preferred_port_name: Option<String>,
    pub action_cc_map: Arc<Mutex<[Option<SequencerAction>; 128]>>,
    pub action_note_map: Arc<Mutex<[Option<SequencerAction>; 128]>>,
}

impl MidiManager {
    pub fn new() -> Self {
        let mut cc_map = [None; 128];
        // Valores de fábrica Nektar Impact LX49+
        cc_map[106] = Some(SequencerAction::TogglePlay);       // Play / Pause
        cc_map[105] = Some(SequencerAction::Stop);             // Stop
        cc_map[107] = Some(SequencerAction::ToggleRecord);     // Record Overdub
        cc_map[102] = Some(SequencerAction::TapTempo);        // Tap Tempo
        cc_map[103] = Some(SequencerAction::BpmDown);         // BPM -1
        cc_map[104] = Some(SequencerAction::BpmUp);           // BPM +1
        cc_map[47]  = Some(SequencerAction::ToggleClick);      // B1: Clic Metrónomo
        cc_map[48]  = Some(SequencerAction::CycleResolution);  // B2: Grilla
        cc_map[51]  = Some(SequencerAction::ToggleBank);       // B5: Banco A/B
        cc_map[52]  = Some(SequencerAction::Undo);             // B6: Undo
        cc_map[53]  = Some(SequencerAction::ClearSelectedTrack);// B7: Clear Track
        cc_map[54]  = Some(SequencerAction::ToggleVisualMetronome); // B8: Metrónomo Visual
        cc_map[55]  = Some(SequencerAction::ClearAll);         // B9: Clear Pattern

        let note_map = [None; 128];

        Self {
            _connections: Vec::new(),
            connected_port_index: None,
            connected_port_name: None,
            preferred_port_name: None,
            action_cc_map: Arc::new(Mutex::new(cc_map)),
            action_note_map: Arc::new(Mutex::new(note_map)),
        }
    }

    pub fn set_action_mappings(
        &self,
        cc_mappings: &[(u8, SequencerAction)],
        note_mappings: &[(u8, SequencerAction)],
    ) {
        if let Ok(mut ccs) = self.action_cc_map.lock() {
            *ccs = [None; 128];
            for &(cc, action) in cc_mappings {
                if (cc as usize) < 128 {
                    ccs[cc as usize] = Some(action);
                }
            }
        }
        if let Ok(mut notes) = self.action_note_map.lock() {
            *notes = [None; 128];
            for &(note, action) in note_mappings {
                if (note as usize) < 128 {
                    notes[note as usize] = Some(action);
                }
            }
        }
    }

    pub fn disconnect(&mut self) {
        self._connections.clear();
        self.connected_port_index = None;
        self.connected_port_name = None;
    }

    pub fn is_connected(&self) -> bool {
        !self._connections.is_empty() && self.connected_port_name.is_some()
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
                    let is_connected = self.is_connected()
                        && self.connected_port_name.as_deref() == Some(&name);
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
        let cc_map_clone = self.action_cc_map.clone();
        let note_map_clone = self.action_note_map.clone();

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
                                // Verificar si esta nota física está mapeada a una acción de UI
                                let mut action_handled = false;
                                if (*note as usize) < 128 {
                                    if let Ok(guard) = note_map_clone.try_lock() {
                                        if let Some(action) = guard[*note as usize] {
                                            action_handled = true;
                                            match action {
                                                SequencerAction::ToggleVisualMetronome => {
                                                    let _ = app_clone.emit("toggle-visual-metronome", ());
                                                }
                                                SequencerAction::AdvancePage => {
                                                    let _ = app_clone.emit("sequencer-advance-page", ());
                                                }
                                                SequencerAction::ToggleWorkstationMode => {
                                                    let _ = app_clone.emit("toggle-workstation-mode", ());
                                                }
                                                other => {
                                                    let _ = prod.push(AudioEngineCommand::SequencerExecuteAction(other));
                                                }
                                            }
                                        }
                                    }
                                }

                                if !action_handled {
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

                                // Interceptar acciones de UI dinámicas.
                                // SOTA Controller Handling: Los botones físicos y pulsadores de transporte
                                // envían mensajes momentáneos (valor > 0 / 127 al presionar y 0 al soltar).
                                // Solo disparamos la acción en el flanco de pulsación (*value > 0), ignorando la
                                // liberación del botón (*value == 0) para evitar doble disparo o toggle inmediato al soltar.
                                if *value > 0 && (*controller as usize) < 128 {
                                    if let Ok(guard) = cc_map_clone.try_lock() {
                                        if let Some(action) = guard[*controller as usize] {
                                            match action {
                                                SequencerAction::ToggleVisualMetronome => {
                                                    let _ = app_clone.emit("toggle-visual-metronome", ());
                                                }
                                                SequencerAction::AdvancePage => {
                                                    let _ = app_clone.emit("sequencer-advance-page", ());
                                                }
                                                SequencerAction::ToggleWorkstationMode => {
                                                    let _ = app_clone.emit("toggle-workstation-mode", ());
                                                }
                                                other => {
                                                    let _ = prod.push(AudioEngineCommand::SequencerExecuteAction(other));
                                                }
                                            }
                                        }
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
        self.preferred_port_name = Some(port_name.clone());
        self.connected_port_index = Some(target_idx);
        self.connected_port_name = Some(port_name);
        self._connections.push(conn);

        Ok(self.list_ports())
    }

    pub fn find_best_port_index(&self, port_names: &[String]) -> Option<usize> {
        if port_names.is_empty() {
            return None;
        }

        // 1. Si hay un puerto preferido previo (ej. el que el usuario seleccionó antes de desenchufar)
        if let Some(ref pref) = self.preferred_port_name {
            if let Some(idx) = port_names.iter().position(|n| n == pref) {
                return Some(idx);
            }
        }

        // 2. Si hay un puerto Impact LX49+ / Nektar que no sea secundario (DAW/MIDIIN2)
        for (idx, name) in port_names.iter().enumerate() {
            let nl = name.to_lowercase();
            let is_impact = nl.contains("impact") || nl.contains("lx49") || nl.contains("nektar");
            let is_secondary = nl.contains("midiin2") || nl.contains("daw") || nl.contains("ctrl");
            if is_impact && !is_secondary {
                return Some(idx);
            }
        }

        // 3. Primer puerto que no sea secundario
        for (idx, name) in port_names.iter().enumerate() {
            if !name.to_lowercase().contains("midiin2") {
                return Some(idx);
            }
        }

        Some(0)
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
            self.disconnect();
            return Err("No se encontraron puertos MIDI de entrada en el sistema".to_string());
        }

        let mut port_names = Vec::new();
        for p in &in_ports {
            if let Ok(name) = midi_scan.port_name(p) {
                port_names.push(name);
            }
        }

        let target_idx = self.find_best_port_index(&port_names).unwrap_or(0);
        self.connect_to_port(target_idx, app_handle, audio_producer)
    }

    pub fn start_watchdog(
        manager_arc: Arc<Mutex<MidiManager>>,
        app_handle: AppHandle,
        audio_producer: Arc<Mutex<Producer<AudioEngineCommand>>>,
    ) {
        std::thread::Builder::new()
            .name("midi-watchdog".to_string())
            .spawn(move || {
                loop {
                    // Dormir 1200ms: 0.00% de CPU
                    std::thread::sleep(std::time::Duration::from_millis(1200));

                    let midi_probe = match MidiInput::new("ImpactSampler WatchdogProbe") {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    let in_ports = midi_probe.ports();
                    let mut port_names = Vec::new();
                    for p in &in_ports {
                        if let Ok(name) = midi_probe.port_name(p) {
                            port_names.push(name);
                        }
                    }

                    let mut mgr = match manager_arc.lock() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    // CASO 1: Estaba marcado como conectado
                    if mgr.is_connected() {
                        let active_name = mgr.connected_port_name.clone().unwrap_or_default();
                        let is_still_present = port_names.iter().any(|n| n == &active_name);

                        if !is_still_present {
                            // ¡El dispositivo fue desconectado físicamente!
                            println!("[MIDI Watchdog] Dispositivo desconectado: '{}'", active_name);
                            mgr.disconnect();
                            let ports_list = mgr.list_ports();
                            let _ = app_handle.emit("midi-status-changed", MidiStatusEvent {
                                connected: false,
                                port_name: None,
                                ports: ports_list,
                            });
                        }
                    }
                    // CASO 2: Estaba desconectado pero hay puertos disponibles
                    else if !port_names.is_empty() {
                        if let Some(target_idx) = mgr.find_best_port_index(&port_names) {
                            let target_name = port_names[target_idx].clone();
                            println!("[MIDI Watchdog] Detectado puerto disponible: '{}'. Reconectando automáticamente...", target_name);
                            if let Ok(ports_list) = mgr.connect_to_port(target_idx, app_handle.clone(), audio_producer.clone()) {
                                println!("[MIDI Watchdog] ¡Reconexión automática exitosa a '{}'!", target_name);
                                let _ = app_handle.emit("midi-status-changed", MidiStatusEvent {
                                    connected: true,
                                    port_name: Some(target_name),
                                    ports: ports_list,
                                });
                            }
                        }
                    }
                }
            })
            .expect("Error al iniciar hilo midi-watchdog");
    }
}
