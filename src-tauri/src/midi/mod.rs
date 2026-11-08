pub mod parser;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    /// Época de sesión MIDI vigente al emitir el estado (monotónica; cada conexión y cada
    /// desconexión/invalidación la incrementa). El frontend descarta eventos con época menor (A04).
    #[serde(default)]
    pub session: u64,
}

/// Evento MIDI normalizado más la época de la sesión que lo originó. `flatten` mantiene el
/// formato previo del payload `midi-event` y solo añade `session`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionMidiEvent<'a> {
    #[serde(flatten)]
    event: &'a NormalizedMidiEvent,
    session: u64,
}

/// Identidad de una sesión de conexión MIDI (A04). El callback de `midir` conserva su `id`; al
/// cambiar de puerto o desconectar se publica una época nueva en `current`, de modo que cualquier
/// evento tardío de la sesión anterior se descarta sin tocar el estado de la sesión vigente.
/// Es independiente de `active_stream_generation` (A16), que pertenece al stream de audio.
#[derive(Clone)]
pub struct MidiSessionGate {
    current: Arc<AtomicU64>,
    id: u64,
}

impl MidiSessionGate {
    pub fn new(current: Arc<AtomicU64>, id: u64) -> Self {
        Self { current, id }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn is_current(&self) -> bool {
        self.current.load(Ordering::SeqCst) == self.id
    }
}

/// Registro atómico de notas y pads sostenidos por la interfaz virtual (ratón / teclado del
/// ordenador). Permite que la liberación por desconexión MIDI no silencie una nota o pad que
/// otra fuente (A12) mantiene sostenido. Lectura/escritura sin bloqueo.
pub struct VirtualHolds {
    notes: [AtomicBool; 128],
    pads: [AtomicBool; 8],
}

impl VirtualHolds {
    pub fn new() -> Self {
        Self {
            notes: std::array::from_fn(|_| AtomicBool::new(false)),
            pads: std::array::from_fn(|_| AtomicBool::new(false)),
        }
    }

    pub fn set_note(&self, note: u8, held: bool) {
        if (note as usize) < 128 {
            self.notes[note as usize].store(held, Ordering::SeqCst);
        }
    }

    pub fn note_held(&self, note: u8) -> bool {
        (note as usize) < 128 && self.notes[note as usize].load(Ordering::SeqCst)
    }

    pub fn set_pad(&self, pad_index: usize, held: bool) {
        if pad_index < 8 {
            self.pads[pad_index].store(held, Ordering::SeqCst);
        }
    }

    pub fn pad_held(&self, pad_index: usize) -> bool {
        pad_index < 8 && self.pads[pad_index].load(Ordering::SeqCst)
    }
}

/// Conjunto de notas (bitset 128) y pads (bitset 8) pendientes de liberación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReleaseSet {
    pub notes: [u64; 2],
    pub pads: u8,
}

impl ReleaseSet {
    pub fn is_empty(&self) -> bool {
        self.notes == [0, 0] && self.pads == 0
    }

    pub fn add_note(&mut self, note: u8) {
        if note < 128 {
            self.notes[(note / 64) as usize] |= 1u64 << (note % 64);
        }
    }

    #[allow(dead_code)]
    pub fn has_note(&self, note: u8) -> bool {
        note < 128 && (self.notes[(note / 64) as usize] >> (note % 64)) & 1 == 1
    }

    pub fn remove_note(&mut self, note: u8) {
        if note < 128 {
            self.notes[(note / 64) as usize] &= !(1u64 << (note % 64));
        }
    }

    pub fn add_pad(&mut self, pad_index: usize) {
        if pad_index < 8 {
            self.pads |= 1 << pad_index;
        }
    }

    #[allow(dead_code)]
    pub fn has_pad(&self, pad_index: usize) -> bool {
        pad_index < 8 && (self.pads >> pad_index) & 1 == 1
    }

    pub fn remove_pad(&mut self, pad_index: usize) {
        if pad_index < 8 {
            self.pads &= !(1 << pad_index);
        }
    }

    pub fn merge(&mut self, other: &ReleaseSet) {
        self.notes[0] |= other.notes[0];
        self.notes[1] |= other.notes[1];
        self.pads |= other.pads;
    }

    pub fn command(&self) -> AudioEngineCommand {
        AudioEngineCommand::ReleaseNotes {
            notes: self.notes,
            pads: self.pads,
        }
    }
}

/// Vacía el registro de notas activas de la sesión y devuelve lo que debe liberarse, omitiendo
/// las notas/pads que la interfaz virtual sigue sosteniendo. Una nota presente en varios canales
/// se libera una sola vez (sin `NoteOff` duplicados).
pub fn drain_session_release_set(
    active_note_pads: &Mutex<[Option<u8>; 2048]>,
    holds: &VirtualHolds,
) -> ReleaseSet {
    let mut set = ReleaseSet::default();
    let mut guard = match active_note_pads.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    for (slot, entry) in guard.iter_mut().enumerate() {
        if let Some(code) = entry.take() {
            if code == 0 {
                let note = (slot % 128) as u8;
                if !holds.note_held(note) {
                    set.add_note(note);
                }
            } else {
                let pad_index = (code as usize).saturating_sub(1);
                if !holds.pad_held(pad_index) {
                    set.add_pad(pad_index);
                }
            }
        }
    }
    set
}

/// Encola `set` como un único `ReleaseNotes`. Reintenta fuera del callback de audio (hilo MIDI/IPC)
/// mientras la cola esté llena; si agota los intentos devuelve el conjunto para reintentarlo luego.
pub fn deliver_release_set(
    audio_producer: &Mutex<Producer<AudioEngineCommand>>,
    set: ReleaseSet,
    attempts: u32,
    wait: std::time::Duration,
) -> Result<(), ReleaseSet> {
    if set.is_empty() {
        return Ok(());
    }
    for attempt in 0..attempts.max(1) {
        {
            let mut prod = match audio_producer.lock() {
                Ok(p) => p,
                Err(poisoned) => poisoned.into_inner(),
            };
            if prod.push(set.command()).is_ok() {
                return Ok(());
            }
        }
        if attempt + 1 < attempts {
            std::thread::sleep(wait);
        }
    }
    Err(set)
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
    /// Época de la sesión MIDI vigente (A04). Los callbacks de sesiones anteriores quedan obsoletos.
    pub current_epoch: Arc<AtomicU64>,
    epoch_counter: u64,
    /// Notas/pads sostenidos desde la interfaz virtual; nunca se liberan por desconexión MIDI.
    pub virtual_holds: Arc<VirtualHolds>,
    /// Productor hacia el motor, conservado para liberar notas en `disconnect()`.
    release_producer: Option<Arc<Mutex<Producer<AudioEngineCommand>>>>,
    /// Liberación no entregada (cola saturada o motor detenido); se reintenta sin bloquear.
    pending_release: ReleaseSet,
    app_handle: Option<AppHandle>,
}

/// Normaliza el nombre de un puerto MIDI eliminando sufijos variables de hardware/ALSA.
/// En Linux/ALSA a través de `midir`, los nombres tienen el formato:
/// `"<client_name>:<port_name> <client_id>:<port_id>"` (ej. `"Impact LX49+:Impact LX49+ MIDI1 24:0"`).
/// Al desconectar y reconectar, `<client_id>` cambia (ej. `24:0` -> `28:0`).
/// Esta función extrae la identidad estable: `"Impact LX49+:Impact LX49+ MIDI1"`.
/// En plataformas sin ese sufijo (macOS / Windows), devuelve la cadena limpia.
pub fn normalize_port_name(name: &str) -> &str {
    let trimmed = name.trim();
    if let Some(pos) = trimmed.rfind(' ') {
        let suffix = &trimmed[pos + 1..];
        if let Some((client_str, port_str)) = suffix.split_once(':') {
            if !client_str.is_empty()
                && client_str.chars().all(|c| c.is_ascii_digit())
                && !port_str.is_empty()
                && port_str.chars().all(|c| c.is_ascii_digit())
            {
                return trimmed[..pos].trim_end();
            }
        }
    }
    trimmed
}

/// Identifica si un puerto MIDI es virtual interno (ej. "Midi Through" en Linux ALSA)
/// o un subpuerto secundario de control (DAW / CTRL / MIDIIN2) que no debe ser
/// autoseleccionado como controlador primario de interpretación musical.
pub fn is_virtual_or_secondary_port(name: &str) -> bool {
    let nl = name.to_lowercase();
    nl.contains("through")
        || nl.contains("midiin2")
        || nl.contains("daw")
        || nl.contains("ctrl")
}

/// Busca en `port_names` el índice del puerto que corresponde inequívocamente al dispositivo
/// que se desea recuperar (`target_name`).
/// 
/// Reglas estrictas:
/// 1. Coincidencia exacta de nombre completo (incluyendo ID ALSA idéntico).
/// 2. Coincidencia por identidad normalizada (descartando sufijo variable `client_id:port_id`).
///    - Si exactamente UN puerto en `port_names` tiene la misma identidad normalizada, retorna `Some(idx)`.
///    - Si NINGÚN puerto coincide, retorna `None`.
///    - Si MÁS DE UN puerto coincide con la misma identidad normalizada y ninguno es exacto,
///      se rechaza la ambigüedad y retorna `None` para impedir selecciones arbitrarias.
pub fn find_recovery_port_index(target_name: &str, port_names: &[String]) -> Option<usize> {
    if port_names.is_empty() || target_name.trim().is_empty() {
        return None;
    }

    // 1. Coincidencia exacta de nombre completo
    if let Some(exact_idx) = port_names.iter().position(|p| p == target_name) {
        return Some(exact_idx);
    }

    // 2. Coincidencia por identidad normalizada
    let target_norm = normalize_port_name(target_name);
    let mut matches = Vec::new();
    for (idx, p) in port_names.iter().enumerate() {
        if normalize_port_name(p) == target_norm {
            matches.push(idx);
        }
    }

    if matches.len() == 1 {
        Some(matches[0])
    } else {
        // 0 matches (no disponible) o >1 matches (ambiguo, no elegir arbitrariamente)
        None
    }
}

/// Busca un puerto oficial de fábrica soportado (Nektar / Impact / LX49) que no sea secundario.
pub fn find_factory_default_port_index(port_names: &[String]) -> Option<usize> {
    let mut matches = Vec::new();
    for (idx, name) in port_names.iter().enumerate() {
        let nl = name.to_lowercase();
        let is_impact = nl.contains("impact") || nl.contains("lx49") || nl.contains("nektar");
        if is_impact && !is_virtual_or_secondary_port(name) {
            matches.push(idx);
        }
    }

    if matches.len() == 1 {
        Some(matches[0])
    } else if matches.len() > 1 {
        // Si hay varios que coinciden con Impact, priorizar el puerto primario MIDI1
        for &idx in &matches {
            let nl = port_names[idx].to_lowercase();
            if nl.contains("midi1") || nl.contains("port-0") || nl.contains("port 1") {
                return Some(idx);
            }
        }
        Some(matches[0])
    } else {
        None
    }
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
            current_epoch: Arc::new(AtomicU64::new(0)),
            epoch_counter: 0,
            virtual_holds: Arc::new(VirtualHolds::new()),
            release_producer: None,
            pending_release: ReleaseSet::default(),
            app_handle: None,
        }
    }

    pub fn session_epoch(&self) -> u64 {
        self.current_epoch.load(Ordering::SeqCst)
    }

    /// Publica una época nueva sin dueño: cualquier callback de la sesión previa pasa a obsoleto.
    fn bump_epoch(&mut self) -> u64 {
        self.epoch_counter += 1;
        self.current_epoch.store(self.epoch_counter, Ordering::SeqCst);
        self.epoch_counter
    }

    /// Invalida la sesión vigente, cierra las conexiones y libera de forma selectiva únicamente
    /// las notas/pads de esa sesión (A04). Orden deliberado: 1) invalidar, 2) cerrar (en ALSA
    /// `close` espera al hilo del callback), 3) recolectar el registro, 4) entregar un único
    /// `ReleaseNotes`. Si la cola está saturada la liberación queda pendiente, no se pierde.
    fn invalidate_and_release(&mut self) {
        self.bump_epoch();
        self._connections.clear();
        self.connected_port_index = None;
        self.connected_port_name = None;

        let mut set = drain_session_release_set(&self.active_note_pads, &self.virtual_holds);
        set.merge(&self.pending_release);
        self.pending_release = ReleaseSet::default();
        if set.is_empty() {
            return;
        }
        match &self.release_producer {
            Some(prod) => {
                if let Err(rest) = deliver_release_set(
                    prod,
                    set,
                    Self::RELEASE_ATTEMPTS,
                    Self::RELEASE_WAIT,
                ) {
                    self.pending_release = rest;
                }
            }
            None => self.pending_release = set,
        }
    }

    const RELEASE_ATTEMPTS: u32 = 25;
    const RELEASE_WAIT: std::time::Duration = std::time::Duration::from_millis(2);

    /// Reintenta (sin esperar) una liberación pendiente. Excluye lo que la sesión nueva o la
    /// interfaz virtual sostienen ahora, para no cortar notas de un puerto recién conectado.
    pub fn flush_pending_release(&mut self) {
        if self.pending_release.is_empty() {
            return;
        }
        let mut set = self.pending_release;
        if let Ok(active) = self.active_note_pads.lock() {
            for (slot, entry) in active.iter().enumerate() {
                match entry {
                    Some(0) => set.remove_note((slot % 128) as u8),
                    Some(code) => set.remove_pad((*code as usize).saturating_sub(1)),
                    None => {}
                }
            }
        }
        for note in 0..128u8 {
            if self.virtual_holds.note_held(note) {
                set.remove_note(note);
            }
        }
        for pad in 0..8usize {
            if self.virtual_holds.pad_held(pad) {
                set.remove_pad(pad);
            }
        }
        let Some(prod) = &self.release_producer else {
            return;
        };
        match deliver_release_set(prod, set, 1, std::time::Duration::ZERO) {
            Ok(()) => self.pending_release = ReleaseSet::default(),
            Err(rest) => self.pending_release = rest,
        }
    }

    /// Libera la sesión anterior y abre una nueva con su propio `MidiSessionGate`.
    fn switch_session(
        &mut self,
        audio_producer: Arc<Mutex<Producer<AudioEngineCommand>>>,
    ) -> MidiSessionGate {
        self.release_producer = Some(audio_producer);
        self.invalidate_and_release();
        if let Ok(mut active_guard) = self.active_note_pads.lock() {
            *active_guard = [None; 2048];
        }
        let id = self.bump_epoch();
        MidiSessionGate::new(self.current_epoch.clone(), id)
    }

    fn emit_status(&self) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(
                "midi-status-changed",
                MidiStatusEvent {
                    connected: self.is_connected(),
                    port_name: self.connected_port_name.clone(),
                    ports: self.list_ports(),
                    session: self.session_epoch(),
                },
            );
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
        gate: Option<&MidiSessionGate>,
        mut on_ui_action: F,
    ) -> Option<NormalizedMidiEvent>
    where
        F: FnMut(SequencerAction),
    {
        // A04: un evento tardío de una sesión invalidada no debe tocar el estado vigente.
        if let Some(g) = gate {
            if !g.is_current() {
                return None;
            }
        }
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
            // Re-validación bajo el candado del productor: la liberación de la sesión se encola con
            // este mismo candado, así que ningún NoteOn obsoleto puede colarse detrás de ella.
            if let Some(g) = gate {
                if !g.is_current() {
                    if let MidiEventType::NoteOn { note, .. } = &event.event {
                        if (*note as usize) < 128 {
                            if let Ok(mut active_guard) = active_note_pads.lock() {
                                active_guard[ch_idx * 128 + (*note as usize)] = None;
                            }
                        }
                    }
                    return None;
                }
            }
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
        self.invalidate_and_release();
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
        let was_connected = self.is_connected();
        self.app_handle = Some(app_handle.clone());
        let result = self.connect_to_port_inner(target_idx, app_handle, audio_producer);
        match &result {
            Ok(_) => self.emit_status(),
            // Si se perdió una conexión activa y la nueva falló, la UI debe limpiar su estado.
            Err(_) if was_connected => self.emit_status(),
            Err(_) => {}
        }
        result
    }

    fn connect_to_port_inner(
        &mut self,
        target_idx: usize,
        app_handle: AppHandle,
        audio_producer: Arc<Mutex<Producer<AudioEngineCommand>>>,
    ) -> Result<Vec<MidiPortInfo>, String> {
        // A04: liberar selectivamente la sesión anterior y abrir una nueva antes de tocar midir.
        let gate = self.switch_session(audio_producer.clone());
        let epoch = gate.id();

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
                    Some(&gate),
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
                    let _ = app_clone.emit(
                        "midi-event",
                        &SessionMidiEvent {
                            event: &event,
                            session: epoch,
                        },
                    );
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

        // 1. Si hay un puerto preferido o conectado previamente, buscarlo con recuperación estricta
        if let Some(ref pref) = self.preferred_port_name {
            if let Some(idx) = find_recovery_port_index(pref, port_names) {
                return Some(idx);
            }
        }

        // 2. Si no hay preferencia previa, buscar exclusivamente hardware de fábrica soportado (Impact / Nektar)
        if let Some(idx) = find_factory_default_port_index(port_names) {
            return Some(idx);
        }

        // No autoseleccionar puertos virtuales como "Midi Through" ni controladores arbitrarios
        None
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

        if let Some(target_idx) = self.find_best_port_index(&port_names) {
            self.connect_to_port(target_idx, app_handle, audio_producer)
        } else {
            self.disconnect();
            Ok(self.list_ports())
        }
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

                    // A04: reintentar sin bloquear una liberación que no pudo entregarse
                    mgr.flush_pending_release();

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
                                session: mgr.session_epoch(),
                            });
                        }
                    }
                    // CASO 2: Estaba desconectado pero hay puertos disponibles
                    else if !port_names.is_empty() {
                        // En recuperación automática (A04): si había un dispositivo conectado o preferido,
                        // supervisar EXCLUSIVAMENTE ese dispositivo. NUNCA cambiar a otro controlador
                        // ni autoconectar a Midi Through.
                        let target_idx = if let Some(ref target) = mgr.preferred_port_name {
                            find_recovery_port_index(target, &port_names)
                        } else {
                            find_factory_default_port_index(&port_names)
                        };

                        if let Some(idx) = target_idx {
                            let target_name = port_names[idx].clone();
                            println!("[MIDI Watchdog] Detectado dispositivo esperado: '{}'. Reconectando automáticamente...", target_name);
                            if let Ok(ports_list) = mgr.connect_to_port(idx, app_handle.clone(), audio_producer.clone()) {
                                println!("[MIDI Watchdog] ¡Reconexión automática exitosa a '{}'!", target_name);
                                let _ = app_handle.emit("midi-status-changed", MidiStatusEvent {
                                    connected: true,
                                    port_name: Some(target_name),
                                    ports: ports_list,
                                    session: mgr.session_epoch(),
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
                    None,
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
                    None,
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
                None,
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
                None,
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
                None,
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
            None,
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
            None,
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
            None,
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

    // ---------------------------------------------------------------------------------------
    // A04 — Desconexión MIDI: liberación selectiva por sesión (pruebas unitarias; no requieren
    // hardware MIDI real: la sesión se simula con `switch_session` y `process_midi_bytes`).
    // ---------------------------------------------------------------------------------------

    fn a04_setup(capacity: usize) -> (
        MidiManager,
        Arc<Mutex<Producer<AudioEngineCommand>>>,
        rtrb::Consumer<AudioEngineCommand>,
    ) {
        let (prod, cons) = RingBuffer::<AudioEngineCommand>::new(capacity);
        (MidiManager::new(), Arc::new(Mutex::new(prod)), cons)
    }

    fn a04_send(
        mgr: &MidiManager,
        prod: &Mutex<Producer<AudioEngineCommand>>,
        gate: &MidiSessionGate,
        bytes: &[u8],
    ) -> Option<NormalizedMidiEvent> {
        MidiManager::process_midi_bytes(
            "A04Port",
            bytes,
            &mgr.pad_note_map,
            &mgr.active_note_pads,
            &mgr.action_cc_map,
            &mgr.action_note_map,
            prod,
            Some(gate),
            |_| {},
        )
    }

    fn a04_drain(cons: &mut rtrb::Consumer<AudioEngineCommand>) -> Vec<AudioEngineCommand> {
        let mut out = Vec::new();
        while let Ok(c) = cons.pop() {
            out.push(c);
        }
        out
    }

    fn a04_releases(cmds: &[AudioEngineCommand]) -> Vec<ReleaseSet> {
        cmds.iter()
            .filter_map(|c| match c {
                AudioEngineCommand::ReleaseNotes { notes, pads } => Some(ReleaseSet {
                    notes: *notes,
                    pads: *pads,
                }),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_a04_physical_noteon_without_noteoff_then_disconnect_releases_only_that_note() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        let gate = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate, &[0x90, 60, 100]).unwrap();
        a04_send(&mgr, &prod, &gate, &[0x90, 64, 100]).unwrap();
        a04_send(&mgr, &prod, &gate, &[0x80, 64, 0]).unwrap(); // 64 sí recibió NoteOff
        a04_drain(&mut cons);

        mgr.disconnect();

        let cmds = a04_drain(&mut cons);
        let rel = a04_releases(&cmds);
        assert_eq!(rel.len(), 1, "Una sola liberación atómica");
        assert!(rel[0].has_note(60));
        assert!(!rel[0].has_note(64), "No se libera una nota ya soltada (sin NoteOff duplicado)");
        assert_eq!(rel[0].pads, 0);
        assert!(!gate.is_current(), "La sesión desconectada queda invalidada");
        assert!(!mgr.is_connected());
        // No hay AllNotesOff global ni NoteOff sueltos
        assert!(cmds.iter().all(|c| matches!(c, AudioEngineCommand::ReleaseNotes { .. })));
        // Segunda desconexión: nada que liberar
        mgr.disconnect();
        assert!(a04_drain(&mut cons).is_empty());
    }

    #[test]
    fn test_a04_port_switch_with_held_notes_releases_old_session_and_not_new_one() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        let gate_a = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate_a, &[0x90, 60, 100]).unwrap();
        a04_send(&mgr, &prod, &gate_a, &[0x99, 38, 100]).unwrap(); // Pad 2
        a04_drain(&mut cons);

        // Cambio de puerto
        let gate_b = mgr.switch_session(prod.clone());
        let rel = a04_releases(&a04_drain(&mut cons));
        assert_eq!(rel.len(), 1);
        assert!(rel[0].has_note(60));
        assert!(rel[0].has_pad(1), "Pad 2 (índice 1) liberado");
        assert!(!rel[0].has_note(38), "Un pad no se libera como nota melódica");

        // La sesión nueva toca la 62: una desconexión posterior solo libera la 62
        a04_send(&mgr, &prod, &gate_b, &[0x90, 62, 90]).unwrap();
        a04_drain(&mut cons);
        mgr.disconnect();
        let rel = a04_releases(&a04_drain(&mut cons));
        assert_eq!(rel.len(), 1);
        assert!(rel[0].has_note(62));
        assert!(!rel[0].has_note(60));
    }

    #[test]
    fn test_a04_late_events_from_previous_session_are_ignored() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        let old_gate = mgr.switch_session(prod.clone());
        mgr.disconnect();
        a04_drain(&mut cons);

        // Evento tardío del callback antiguo: ni encola ni registra notas
        assert!(a04_send(&mgr, &prod, &old_gate, &[0x90, 72, 100]).is_none());
        assert!(a04_send(&mgr, &prod, &old_gate, &[0xB0, 7, 100]).is_none());
        assert!(a04_drain(&mut cons).is_empty());
        let leftovers = drain_session_release_set(&mgr.active_note_pads, &mgr.virtual_holds);
        assert!(leftovers.is_empty(), "La sesión obsoleta no deja notas fantasma registradas");

        // Tampoco afecta a la sesión nueva
        let new_gate = mgr.switch_session(prod.clone());
        assert!(a04_send(&mgr, &prod, &old_gate, &[0x90, 73, 100]).is_none());
        assert!(a04_send(&mgr, &prod, &new_gate, &[0x90, 74, 100]).is_some());
        let cmds = a04_drain(&mut cons);
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], AudioEngineCommand::NoteOn { note: 74, .. }));
    }

    #[test]
    fn test_a04_reconnect_same_device_works_and_old_notes_are_not_double_released() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        let gate_1 = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate_1, &[0x90, 60, 100]).unwrap();
        mgr.disconnect();
        assert_eq!(a04_releases(&a04_drain(&mut cons)).len(), 1);

        // Reconexión del mismo controlador: nueva sesión, mismas notas
        let gate_2 = mgr.switch_session(prod.clone());
        assert!(a04_drain(&mut cons).is_empty(), "No se libera nada al reconectar si ya se liberó");
        a04_send(&mgr, &prod, &gate_2, &[0x90, 60, 100]).unwrap();
        let cmds = a04_drain(&mut cons);
        assert!(matches!(cmds[0], AudioEngineCommand::NoteOn { note: 60, .. }));
        a04_send(&mgr, &prod, &gate_2, &[0x80, 60, 0]).unwrap();
        let cmds = a04_drain(&mut cons);
        assert!(matches!(cmds[0], AudioEngineCommand::NoteOff { note: 60 }));
        mgr.disconnect();
        assert!(a04_drain(&mut cons).is_empty());
    }

    #[test]
    fn test_a04_note_shared_between_mouse_and_physical_midi_is_not_released() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        let gate = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate, &[0x90, 60, 100]).unwrap();
        a04_send(&mgr, &prod, &gate, &[0x90, 67, 100]).unwrap();
        mgr.virtual_holds.set_note(60, true); // el ratón también sostiene el Do central
        a04_drain(&mut cons);

        mgr.disconnect();

        let rel = a04_releases(&a04_drain(&mut cons));
        assert_eq!(rel.len(), 1);
        assert!(!rel[0].has_note(60), "La nota sostenida por el ratón sigue sonando (A12)");
        assert!(rel[0].has_note(67));
    }

    #[test]
    fn test_a04_pad_shared_between_ui_and_controller_is_not_released() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        let gate = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate, &[0x99, 36, 100]).unwrap(); // Pad 1
        a04_send(&mgr, &prod, &gate, &[0x99, 38, 100]).unwrap(); // Pad 2
        mgr.virtual_holds.set_pad(0, true); // la interfaz mantiene el Pad 1
        a04_drain(&mut cons);

        mgr.disconnect();

        let rel = a04_releases(&a04_drain(&mut cons));
        assert_eq!(rel.len(), 1);
        assert!(!rel[0].has_pad(0), "Pad sostenido por la UI no se libera");
        assert!(rel[0].has_pad(1));
    }

    #[test]
    fn test_a04_same_note_on_two_channels_is_released_once() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        let gate = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate, &[0x90, 60, 100]).unwrap();
        a04_send(&mgr, &prod, &gate, &[0x91, 60, 100]).unwrap();
        a04_drain(&mut cons);
        mgr.disconnect();
        let rel = a04_releases(&a04_drain(&mut cons));
        assert_eq!(rel.len(), 1);
        assert!(rel[0].has_note(60));
    }

    #[test]
    fn test_a04_saturated_queue_keeps_release_pending_and_flushes_later() {
        // Cola de 2 posiciones llena de comandos previos
        let (mut mgr, prod, mut cons) = a04_setup(2);
        let gate = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate, &[0x90, 60, 100]).unwrap();
        a04_send(&mgr, &prod, &gate, &[0x90, 61, 100]).unwrap();
        // cola llena: no se drena todavía

        mgr.disconnect();
        assert!(
            !mgr.pending_release.is_empty(),
            "Con la cola saturada la liberación debe quedar pendiente, no perderse"
        );
        assert!(mgr.pending_release.has_note(60) && mgr.pending_release.has_note(61));

        // Sin espacio: el flush no entrega ni pierde nada
        mgr.flush_pending_release();
        assert!(!mgr.pending_release.is_empty());

        // El motor drena la cola; el flush entrega un único ReleaseNotes
        a04_drain(&mut cons);
        mgr.flush_pending_release();
        assert!(mgr.pending_release.is_empty());
        let rel = a04_releases(&a04_drain(&mut cons));
        assert_eq!(rel.len(), 1);
        assert!(rel[0].has_note(60) && rel[0].has_note(61));
    }

    #[test]
    fn test_a04_pending_release_does_not_cut_notes_of_new_session_or_virtual_input() {
        let (mut mgr, prod, mut cons) = a04_setup(2);
        let gate = mgr.switch_session(prod.clone());
        for n in [60u8, 61, 62] {
            let _ = a04_send(&mgr, &prod, &gate, &[0x90, n, 100]);
        }
        // Registro: 60, 61, 62 (la cola solo admitió 2, el registro anota las tres)
        mgr.disconnect();
        assert!(!mgr.pending_release.is_empty());

        // Reconexión rápida: el mismo controlador ya toca la 61 y la UI sostiene la 62
        a04_drain(&mut cons);
        let gate2 = mgr.switch_session(prod.clone());
        a04_drain(&mut cons); // descarta la entrega inmediata (si hubo espacio)
        mgr.pending_release = ReleaseSet::default();
        mgr.pending_release.add_note(60);
        mgr.pending_release.add_note(61);
        mgr.pending_release.add_note(62);
        a04_send(&mgr, &prod, &gate2, &[0x90, 61, 100]).unwrap();
        mgr.virtual_holds.set_note(62, true);
        a04_drain(&mut cons);

        mgr.flush_pending_release();

        let rel = a04_releases(&a04_drain(&mut cons));
        assert_eq!(rel.len(), 1);
        assert!(rel[0].has_note(60));
        assert!(!rel[0].has_note(61), "Nota de la sesión nueva intacta");
        assert!(!rel[0].has_note(62), "Nota virtual intacta");
    }

    #[test]
    fn test_a04_rapid_port_switching_keeps_only_latest_session_valid() {
        let (mut mgr, prod, mut cons) = a04_setup(256);
        let mut gates = Vec::new();
        for i in 0..20u8 {
            let g = mgr.switch_session(prod.clone());
            a04_send(&mgr, &prod, &g, &[0x90, 50 + i, 100]).unwrap();
            gates.push(g);
        }
        let total_releases: Vec<ReleaseSet> = a04_releases(&a04_drain(&mut cons));
        // 19 cambios con una nota sostenida cada uno => 19 liberaciones, una por sesión
        assert_eq!(total_releases.len(), 19);
        for (i, r) in total_releases.iter().enumerate() {
            assert!(r.has_note(50 + i as u8), "Cada sesión libera exclusivamente su propia nota");
            let count: u32 = r.notes.iter().map(|w| w.count_ones()).sum();
            assert_eq!(count, 1);
        }
        let valid: Vec<bool> = gates.iter().map(|g| g.is_current()).collect();
        assert_eq!(valid.iter().filter(|v| **v).count(), 1);
        assert!(*valid.last().unwrap());
        assert!(gates.windows(2).all(|w| w[0].id() < w[1].id()), "Épocas monotónicas");
    }

    #[test]
    fn test_a04_session_epoch_is_independent_and_monotonic() {
        let (mut mgr, prod, _cons) = a04_setup(8);
        assert_eq!(mgr.session_epoch(), 0);
        let g1 = mgr.switch_session(prod.clone());
        let e1 = mgr.session_epoch();
        mgr.disconnect();
        let e2 = mgr.session_epoch();
        let g2 = mgr.switch_session(prod.clone());
        assert!(g1.id() == e1 && e2 > e1 && g2.id() > e2);
    }

    // ---------------------------------------------------------------------------------------
    // A04 (Recovery): Pruebas de identidad, normalización ALSA y no-reconexión a puertos erróneos
    // ---------------------------------------------------------------------------------------

    #[test]
    fn test_a04_recovery_normalization_port_names() {
        assert_eq!(
            normalize_port_name("Impact LX49+:Impact LX49+ MIDI1 24:0"),
            "Impact LX49+:Impact LX49+ MIDI1"
        );
        assert_eq!(
            normalize_port_name("Impact LX49+:Impact LX49+ MIDI1 28:0"),
            "Impact LX49+:Impact LX49+ MIDI1"
        );
        assert_eq!(
            normalize_port_name("Midi Through:Midi Through Port-0 14:0"),
            "Midi Through:Midi Through Port-0"
        );
        assert_eq!(
            normalize_port_name("Arturia KeyStep:KeyStep MIDI 26:0"),
            "Arturia KeyStep:KeyStep MIDI"
        );
        assert_eq!(
            normalize_port_name("Impact LX49+"),
            "Impact LX49+"
        );
        assert_eq!(
            normalize_port_name("Port: 1"),
            "Port: 1"
        );
        assert_eq!(
            normalize_port_name(""),
            ""
        );
    }

    #[test]
    fn test_a04_disconnect_impact_when_midi_through_available_does_not_connect_through() {
        let mut mgr = MidiManager::new();
        // Dispositivo previamente conectado / preferido
        mgr.preferred_port_name = Some("Impact LX49+:Impact LX49+ MIDI1 24:0".to_string());

        // Al desconectar el cable USB, sólo queda disponible Midi Through en ALSA
        let available_ports = vec!["Midi Through:Midi Through Port-0 14:0".to_string()];

        // 1. find_recovery_port_index debe retornar None
        let rec = find_recovery_port_index(
            mgr.preferred_port_name.as_ref().unwrap(),
            &available_ports,
        );
        assert_eq!(rec, None, "No debe recuperar Midi Through cuando se busca Impact LX49+");

        // 2. find_best_port_index tampoco debe conectar a Midi Through
        let best = mgr.find_best_port_index(&available_ports);
        assert_eq!(best, None, "El watchdog / auto-connect NO debe autoseleccionar Midi Through");
    }

    #[test]
    fn test_a04_reconnect_impact_with_different_alsa_index() {
        let mut mgr = MidiManager::new();
        // Sesión anterior estaba en cliente ALSA 24:0
        mgr.preferred_port_name = Some("Impact LX49+:Impact LX49+ MIDI1 24:0".to_string());

        // Al reconectar el cable USB, el kernel ALSA le asigna client_id 28:0
        let available_ports = vec![
            "Midi Through:Midi Through Port-0 14:0".to_string(),
            "Impact LX49+:Impact LX49+ MIDI1 28:0".to_string(),
        ];

        // find_recovery_port_index debe encontrar exactamente el índice 1 mediante normalización ALSA
        let rec = find_recovery_port_index(
            mgr.preferred_port_name.as_ref().unwrap(),
            &available_ports,
        );
        assert_eq!(rec, Some(1), "Debe reconocer el mismo dispositivo físico con nuevo client ID ALSA 28:0");

        let best = mgr.find_best_port_index(&available_ports);
        assert_eq!(best, Some(1), "find_best_port_index debe priorizar el hardware reconectado");
    }

    #[test]
    fn test_a04_manual_selection_updates_preference_and_allows_midi_through() {
        let mut mgr = MidiManager::new();
        // Si el usuario en la UI selecciona manualmente Midi Through
        let manual_choice = "Midi Through:Midi Through Port-0 14:0".to_string();
        mgr.preferred_port_name = Some(manual_choice.clone());

        let available_ports = vec![
            "Midi Through:Midi Through Port-0 14:0".to_string(),
            "Impact LX49+:Impact LX49+ MIDI1 24:0".to_string(),
        ];

        // La selección manual del usuario debe ser respetada
        let rec = find_recovery_port_index(&manual_choice, &available_ports);
        assert_eq!(rec, Some(0), "Si el usuario eligió manualmente Midi Through, se debe permitir");
    }

    #[test]
    fn test_a04_two_controllers_available_does_not_switch_to_other_controller() {
        let mut mgr = MidiManager::new();
        mgr.preferred_port_name = Some("Impact LX49+:Impact LX49+ MIDI1 24:0".to_string());

        // El Nektar está desenchufado, pero hay otro teclado USB (ej. Arturia KeyStep) conectado
        let available_ports = vec![
            "Midi Through:Midi Through Port-0 14:0".to_string(),
            "Arturia KeyStep:KeyStep MIDI 26:0".to_string(),
        ];

        // No debe conectarse automáticamente a Arturia ni a Midi Through
        let rec = find_recovery_port_index(
            mgr.preferred_port_name.as_ref().unwrap(),
            &available_ports,
        );
        assert_eq!(rec, None, "No debe secuestrar ni conectar a otro controlador distinto al desconectarse el preferido");

        let best = mgr.find_best_port_index(&available_ports);
        assert_eq!(best, None, "Debe permanecer desconectado esperando el dispositivo original");
    }

    #[test]
    fn test_a04_avoid_ambiguous_name_matching() {
        let target = "Generic USB MIDI 20:0";
        // Dos puertos con la misma identidad base normalizada pero distintos IDs
        let available_ports = vec![
            "Generic USB MIDI 24:0".to_string(),
            "Generic USB MIDI 25:0".to_string(),
        ];

        // Como hay 2 coincidencias y ninguna exacta, no debe elegir arbitrariamente
        let rec = find_recovery_port_index(target, &available_ports);
        assert_eq!(rec, None, "Debe rechazar coincidencias ambiguas múltiples");
    }

    #[test]
    fn test_a04_disconnect_held_note_released_and_reconnect_does_not_reactivate_old_notes() {
        let (mut mgr, prod, mut cons) = a04_setup(64);
        mgr.preferred_port_name = Some("Impact LX49+:Impact LX49+ MIDI1 24:0".to_string());

        // Conectar sesión 1
        let gate_1 = mgr.switch_session(prod.clone());
        a04_send(&mgr, &prod, &gate_1, &[0x90, 60, 100]).unwrap(); // Nota sostenida
        a04_drain(&mut cons);

        // Se desconecta el dispositivo (watchdog detecta desconexión)
        mgr.disconnect();

        // Verificar que ReleaseNotes liberó la nota 60 en el motor
        let cmds = a04_drain(&mut cons);
        let rel = a04_releases(&cmds);
        assert_eq!(rel.len(), 1);
        assert!(rel[0].has_note(60));

        // Reaparece el Nektar con nuevo client ID ALSA 28:0 y se reconecta
        let available = vec![
            "Midi Through:Midi Through Port-0 14:0".to_string(),
            "Impact LX49+:Impact LX49+ MIDI1 28:0".to_string(),
        ];
        let recovery_idx = find_recovery_port_index(
            mgr.preferred_port_name.as_ref().unwrap(),
            &available,
        ).expect("Debe encontrar el puerto reconectado");
        assert_eq!(recovery_idx, 1);

        // Se abre la nueva sesión tras reconexión
        let gate_2 = mgr.switch_session(prod.clone());
        assert!(gate_2.is_current());
        assert!(!gate_1.is_current());

        // Comprobar que en la nueva sesión la nota 60 NO está activa
        let leftovers = drain_session_release_set(&mgr.active_note_pads, &mgr.virtual_holds);
        assert!(leftovers.is_empty(), "La nueva sesión no debe heredar notas activas de la sesión previa");

        // Eventos tardíos de la sesión 1 deben ser completamente descartados
        assert!(a04_send(&mgr, &prod, &gate_1, &[0x80, 60, 0]).is_none());
        assert!(a04_drain(&mut cons).is_empty());
    }
}
