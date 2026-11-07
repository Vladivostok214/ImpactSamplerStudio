pub mod parser;

use std::sync::{Arc, Mutex};
use midir::{Ignore, MidiInput, MidiInputConnection};
use rtrb::Producer;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::audio::AudioEngineCommand;
use parser::{
    parse_midi_message_with_pad_map, MidiEventType, NormalizedMidiEvent, DEFAULT_PAD_INPUT_NOTES,
};

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
    /// Mapa dinámico de notas MIDI físicas de entrada para los 8 pads (independiente de la nota SF2 de salida).
    pub pad_note_map: Arc<Mutex<[u8; 8]>>,
    /// Registro de notas activas por canal/nota (`(channel - 1) * 128 + note`):
    /// - `None`: la nota no está activa actualmente.
    /// - `Some(0)`: la nota fue iniciada como nota melódica (`NoteOn`).
    /// - `Some(1..=8)`: la nota fue iniciada como disparo del pad `1..=8` (`PadTrigger`).
    /// Garantiza que `NoteOff` libere exactamente el mismo destino que inició `NoteOn`,
    /// incluso si el mapeo de MIDI Learn cambia mientras la nota sigue presionada.
    pub active_note_pads: Arc<Mutex<[Option<u8>; 2048]>>,
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
            pad_note_map: Arc::new(Mutex::new(DEFAULT_PAD_INPUT_NOTES)),
            active_note_pads: Arc::new(Mutex::new([None; 2048])),
        }
    }

    pub fn set_pad_note_mappings(&self, pads: [u8; 8]) {
        if let Ok(mut guard) = self.pad_note_map.lock() {
            *guard = pads;
        }
    }

    #[allow(dead_code)]
    pub fn get_pad_note_mappings(&self) -> [u8; 8] {
        if let Ok(guard) = self.pad_note_map.lock() {
            *guard
        } else {
            DEFAULT_PAD_INPUT_NOTES
        }
    }

    /// Procesa un paquete MIDI crudo, resolviendo el enrutamiento de pads con `pad_notes`,
    /// preservando la coherencia `NoteOn`/`NoteOff` ante cambios de mapeo en caliente y
    /// encolando el comando correspondiente en `audio_producer`.
    pub fn process_midi_bytes<F>(
        port_name: &str,
        bytes: &[u8],
        pad_note_map: &Mutex<[u8; 8]>,
        active_note_pads: &Mutex<[Option<u8>; 2048]>,
        action_cc_map: &Mutex<[Option<SequencerAction>; 128]>,
        action_note_map: &Mutex<[Option<SequencerAction>; 128]>,
        audio_producer: &Mutex<Producer<AudioEngineCommand>>,
        mut on_ui_action: F,
    ) -> Option<NormalizedMidiEvent>
    where
        F: FnMut(SequencerAction),
    {
        let current_pad_notes = match pad_note_map.try_lock() {
            Ok(guard) => *guard,
            Err(_) => match pad_note_map.lock() {
                Ok(guard) => *guard,
                Err(_) => DEFAULT_PAD_INPUT_NOTES,
            },
        };

        let mut event = parse_midi_message_with_pad_map(port_name, bytes, &current_pad_notes)?;
        let ch_idx = (event.channel.saturating_sub(1) as usize).min(15);

        // Coherencia NoteOn / NoteOff por (canal, nota)
        match &mut event.event {
            MidiEventType::NoteOn {
                note,
                pad_index,
                ..
            } => {
                if (*note as usize) < 128 {
                    let slot = ch_idx * 128 + (*note as usize);
                    let encoded = Some(pad_index.unwrap_or(0));
                    if let Ok(mut active_guard) = active_note_pads.try_lock() {
                        active_guard[slot] = encoded;
                    } else if let Ok(mut active_guard) = active_note_pads.lock() {
                        active_guard[slot] = encoded;
                    }
                }
            }
            MidiEventType::NoteOff {
                note,
                pad_index,
                ..
            } => {
                if (*note as usize) < 128 {
                    let slot = ch_idx * 128 + (*note as usize);
                    let tracked = if let Ok(mut active_guard) = active_note_pads.try_lock() {
                        active_guard[slot].take()
                    } else if let Ok(mut active_guard) = active_note_pads.lock() {
                        active_guard[slot].take()
                    } else {
                        None
                    };
                    if let Some(code) = tracked {
                        *pad_index = if code == 0 { None } else { Some(code) };
                    }
                }
            }
            _ => {}
        }

        if let Ok(mut prod) = audio_producer.lock() {
            match &event.event {
                MidiEventType::NoteOn {
                    note,
                    velocity,
                    pad_index,
                    ..
                } => {
                    let mut action_handled = false;
                    if (*note as usize) < 128 {
                        if let Ok(guard) = action_note_map.try_lock() {
                            if let Some(action) = guard[*note as usize] {
                                action_handled = true;
                                match action {
                                    SequencerAction::ToggleVisualMetronome
                                    | SequencerAction::AdvancePage
                                    | SequencerAction::ToggleWorkstationMode => {
                                        on_ui_action(action);
                                    }
                                    other => {
                                        let _ = prod.push(
                                            AudioEngineCommand::SequencerExecuteAction(other),
                                        );
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
                MidiEventType::NoteOff {
                    note, pad_index, ..
                } => {
                    if let Some(p_idx) = pad_index {
                        let _ = prod.push(AudioEngineCommand::PadRelease {
                            pad_index: (*p_idx as usize).saturating_sub(1),
                        });
                    } else {
                        let _ = prod.push(AudioEngineCommand::NoteOff { note: *note });
                    }
                }
                MidiEventType::ControlChange {
                    controller, value, ..
                } => {
                    let val_f = *value as f32 / 127.0;
                    let _ = prod.push(AudioEngineCommand::ControlChange {
                        controller: *controller,
                        value: val_f,
                    });

                    if *value > 0 && (*controller as usize) < 128 {
                        if let Ok(guard) = action_cc_map.try_lock() {
                            if let Some(action) = guard[*controller as usize] {
                                match action {
                                    SequencerAction::ToggleVisualMetronome
                                    | SequencerAction::AdvancePage
                                    | SequencerAction::ToggleWorkstationMode => {
                                        on_ui_action(action);
                                    }
                                    other => {
                                        let _ = prod.push(
                                            AudioEngineCommand::SequencerExecuteAction(other),
                                        );
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

        Some(event)
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
        let pad_map_clone = self.pad_note_map.clone();
        let active_note_pads_clone = self.active_note_pads.clone();

        if let Ok(mut active_guard) = self.active_note_pads.lock() {
            *active_guard = [None; 2048];
        }

        let conn = midi_in.connect(
            port,
            "impact-primary-input",
            move |_stamp, bytes, _| {
                if let Some(event) = Self::process_midi_bytes(
                    &p_name,
                    bytes,
                    &pad_map_clone,
                    &active_note_pads_clone,
                    &cc_map_clone,
                    &note_map_clone,
                    &prod_clone,
                    |action| match action {
                        SequencerAction::ToggleVisualMetronome => {
                            let _ = app_clone.emit("toggle-visual-metronome", ());
                        }
                        SequencerAction::AdvancePage => {
                            let _ = app_clone.emit("sequencer-advance-page", ());
                        }
                        SequencerAction::ToggleWorkstationMode => {
                            let _ = app_clone.emit("toggle-workstation-mode", ());
                        }
                        _ => {}
                    },
                ) {
                    let _ = app_clone.emit("midi-event", &event);
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

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    #[test]
    fn test_a11_default_eight_pads_routing_and_velocities() {
        let mgr = MidiManager::new();
        let (prod, mut cons) = RingBuffer::<AudioEngineCommand>::new(128);
        let prod_mutex = Mutex::new(prod);

        let expected_notes = DEFAULT_PAD_INPUT_NOTES; // [36, 38, 42, 46, 39, 45, 48, 49]
        let velocities: [u8; 5] = [1, 30, 50, 90, 127];

        for (pad_idx, &note) in expected_notes.iter().enumerate() {
            for &vel in &velocities {
                // NoteOn en Canal 10 (status 0x99)
                let ev = MidiManager::process_midi_bytes(
                    "TestPort",
                    &[0x99, note, vel],
                    &mgr.pad_note_map,
                    &mgr.active_note_pads,
                    &mgr.action_cc_map,
                    &mgr.action_note_map,
                    &prod_mutex,
                    |_| {},
                )
                .expect("Debe parsear NoteOn");

                match ev.event {
                    MidiEventType::NoteOn {
                        pad_index,
                        velocity,
                        ..
                    } => {
                        assert_eq!(pad_index, Some((pad_idx + 1) as u8));
                        assert_eq!(velocity, vel);
                    }
                    _ => panic!("Se esperaba NoteOn"),
                }

                // Verificar que sólo se encoló exactamente 1 comando PadTrigger (sin duplicados)
                let cmd = cons.pop().expect("Debe haber comando PadTrigger");
                match cmd {
                    AudioEngineCommand::PadTrigger {
                        pad_index: triggered_idx,
                        velocity: triggered_vel,
                    } => {
                        assert_eq!(triggered_idx, pad_idx);
                        assert!((triggered_vel - (vel as f32 / 127.0)).abs() < 1e-5);
                    }
                    other => panic!("Se esperaba PadTrigger, se obtuvo {:?}", other),
                }
                assert!(cons.pop().is_err(), "No debe haber disparos duplicados");

                // NoteOff en Canal 10 (status 0x89)
                let off_ev = MidiManager::process_midi_bytes(
                    "TestPort",
                    &[0x89, note, 0],
                    &mgr.pad_note_map,
                    &mgr.active_note_pads,
                    &mgr.action_cc_map,
                    &mgr.action_note_map,
                    &prod_mutex,
                    |_| {},
                )
                .expect("Debe parsear NoteOff");

                match off_ev.event {
                    MidiEventType::NoteOff { pad_index, .. } => {
                        assert_eq!(pad_index, Some((pad_idx + 1) as u8));
                    }
                    _ => panic!("Se esperaba NoteOff"),
                }

                match cons.pop().expect("Debe haber comando PadRelease") {
                    AudioEngineCommand::PadRelease {
                        pad_index: released_idx,
                    } => {
                        assert_eq!(released_idx, pad_idx);
                    }
                    other => panic!("Se esperaba PadRelease, se obtuvo {:?}", other),
                }
                assert!(cons.pop().is_err());
            }
        }
    }

    #[test]
    fn test_a11_custom_midi_learn_notes_across_channels_and_melodic_protection() {
        let mgr = MidiManager::new();
        let (prod, mut cons) = RingBuffer::<AudioEngineCommand>::new(64);
        let prod_mutex = Mutex::new(prod);

        // 1. Con el mapa por defecto [36, 38, 42, 46, 39, 45, 48, 49]:
        // - Las notas que NO pertenecen al mapa de pads (ej. 50, 52, 60, 64, 72) son teclado melódico (NoteOn)
        for &melodic_note in &[50u8, 52, 60, 64, 72] {
            let ev = MidiManager::process_midi_bytes(
                "TestPort",
                &[0x90, melodic_note, 90], // Canal 1
                &mgr.pad_note_map,
                &mgr.active_note_pads,
                &mgr.action_cc_map,
                &mgr.action_note_map,
                &prod_mutex,
                |_| {},
            )
            .unwrap();
            match ev.event {
                MidiEventType::NoteOn { pad_index, .. } => assert_eq!(pad_index, None),
                _ => panic!("Se esperaba NoteOn melódico"),
            }
            match cons.pop().unwrap() {
                AudioEngineCommand::NoteOn { note, .. } => assert_eq!(note, melodic_note),
                other => panic!("Se esperaba NoteOn melódico, se recibió {:?}", other),
            }
        }

        // - El Pad 1 (nota 36) funciona tanto en Canal 1 (0x90) como en Canal 10 (0x99)
        for &status in &[0x90u8, 0x99u8] {
            let ev_pad1 = MidiManager::process_midi_bytes(
                "TestPort",
                &[status, 36, 90],
                &mgr.pad_note_map,
                &mgr.active_note_pads,
                &mgr.action_cc_map,
                &mgr.action_note_map,
                &prod_mutex,
                |_| {},
            )
            .unwrap();
            match ev_pad1.event {
                MidiEventType::NoteOn { pad_index, .. } => assert_eq!(pad_index, Some(1)),
                _ => panic!("Se esperaba NoteOn para Pad 1"),
            }
            match cons.pop().unwrap() {
                AudioEngineCommand::PadTrigger { pad_index, .. } => assert_eq!(pad_index, 0),
                other => panic!("Pad 1 (nota 36) debe disparar PadTrigger en cualquier canal: {:?}", other),
            }
        }

        // 2. El usuario asigna mediante MIDI Learn notas personalizadas a los pads (ej. Pad 1 -> 60, Pad 4 -> 64)
        let mut custom_pads = DEFAULT_PAD_INPUT_NOTES;
        custom_pads[0] = 60; // Pad 1 asignado a C4
        custom_pads[3] = 64; // Pad 4 asignado a E4
        mgr.set_pad_note_mappings(custom_pads);

        // Entrada por Canal 1 (0x90) y por Canal 10 (0x99) para las notas aprendidas debe disparar Pad 1 y Pad 4
        for &(status, note, expected_pad) in &[(0x90u8, 60u8, 0usize), (0x99u8, 60u8, 0usize), (0x92u8, 64u8, 3usize)] {
            let ev = MidiManager::process_midi_bytes(
                "TestPort",
                &[status, note, 50],
                &mgr.pad_note_map,
                &mgr.active_note_pads,
                &mgr.action_cc_map,
                &mgr.action_note_map,
                &prod_mutex,
                |_| {},
            )
            .unwrap();
            match ev.event {
                MidiEventType::NoteOn { pad_index, .. } => {
                    assert_eq!(pad_index, Some((expected_pad + 1) as u8));
                }
                _ => panic!("Se esperaba NoteOn de pad"),
            }
            match cons.pop().unwrap() {
                AudioEngineCommand::PadTrigger { pad_index, .. } => {
                    assert_eq!(pad_index, expected_pad);
                }
                other => panic!("Se esperaba PadTrigger, se obtuvo {:?}", other),
            }
        }

        // 3. Como Pad 1 fue reasignado a 60, su antigua nota por defecto (36) ya no dispara Pad 1 por fallback en Canal 10
        let ev_old = MidiManager::process_midi_bytes(
            "TestPort",
            &[0x99, 36, 90],
            &mgr.pad_note_map,
            &mgr.active_note_pads,
            &mgr.action_cc_map,
            &mgr.action_note_map,
            &prod_mutex,
            |_| {},
        )
        .unwrap();
        match ev_old.event {
            MidiEventType::NoteOn { pad_index, .. } => assert_eq!(pad_index, None),
            _ => panic!("Se esperaba NoteOn"),
        }
        match cons.pop().unwrap() {
            AudioEngineCommand::NoteOn { note: 36, .. } => {}
            other => panic!("La nota antigua 36 no debía disparar Pad 1 tras reasignación: {:?}", other),
        }
    }

    #[test]
    fn test_a11_duplicate_assignment_policy_and_noteon_noteoff_coherence_on_live_remap() {
        let mgr = MidiManager::new();
        let (prod, mut cons) = RingBuffer::<AudioEngineCommand>::new(64);
        let prod_mutex = Mutex::new(prod);

        // Política ante duplicados: si dos pads tienen la misma nota (ej. Pad 2 y Pad 5 con nota 70),
        // gana el menor índice (Pad 2 -> pad_index = 1) y jamás se emiten dos disparos.
        let mut dup_pads = DEFAULT_PAD_INPUT_NOTES;
        dup_pads[1] = 70;
        dup_pads[4] = 70;
        mgr.set_pad_note_mappings(dup_pads);

        MidiManager::process_midi_bytes(
            "TestPort",
            &[0x90, 70, 90],
            &mgr.pad_note_map,
            &mgr.active_note_pads,
            &mgr.action_cc_map,
            &mgr.action_note_map,
            &prod_mutex,
            |_| {},
        )
        .unwrap();

        match cons.pop().unwrap() {
            AudioEngineCommand::PadTrigger { pad_index, .. } => assert_eq!(pad_index, 1),
            other => panic!("Se esperaba PadTrigger en pad 1, se obtuvo {:?}", other),
        }
        assert!(cons.pop().is_err(), "No debe disparar dos pads con una misma nota");

        // Cambio de asignación mientras la nota 70 sigue activa: ahora Pad 2 cambia a nota 72 y Pad 5 a 75
        let mut new_pads = DEFAULT_PAD_INPUT_NOTES;
        new_pads[1] = 72;
        new_pads[4] = 75;
        mgr.set_pad_note_mappings(new_pads);

        // Al soltar la nota 70 (NoteOff), debe liberar el Pad 2 (pad_index = 1) que fue el que se activó en NoteOn
        let off_ev = MidiManager::process_midi_bytes(
            "TestPort",
            &[0x80, 70, 0],
            &mgr.pad_note_map,
            &mgr.active_note_pads,
            &mgr.action_cc_map,
            &mgr.action_note_map,
            &prod_mutex,
            |_| {},
        )
        .unwrap();
        match off_ev.event {
            MidiEventType::NoteOff { pad_index, .. } => assert_eq!(pad_index, Some(2)),
            _ => panic!("Se esperaba NoteOff"),
        }
        match cons.pop().unwrap() {
            AudioEngineCommand::PadRelease { pad_index } => assert_eq!(pad_index, 1),
            other => panic!("NoteOff debe liberar el pad original (1), se obtuvo {:?}", other),
        }
    }
}
