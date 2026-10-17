use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig, SupportedStreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use tauri::{AppHandle, Emitter};

use super::arpeggiator::{Arpeggiator, ArpOutputEvent, ArpStateInfo};
use super::fx::{AudioFxRack, FxRouting, KnobMappingInfo, KnobTarget};
use super::sampler::{PadEngineMode, PadPlaybackMode, PadSampleData, PadSampler, PadSlotInfo};
use super::sequencer::{SequencerAction, SequencerStateInfo, SequencerStepEvent, StepSequencer};
use super::{AudioDeviceInfo, AudioEngineCommand, SequencerUiEvent};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedAudioPreferences {
    pub host: String,
    pub device_name: Option<String>,
    pub channel_pair: usize,
    pub buffer_size: Option<u32>,
    pub sample_rate: Option<u32>,
}

pub fn get_audio_preferences_path() -> Option<std::path::PathBuf> {
    if let Ok(app_data) = std::env::var("APPDATA") {
        let dir = std::path::PathBuf::from(app_data).join("ImpactSamplerStudio");
        let _ = std::fs::create_dir_all(&dir);
        Some(dir.join("audio_preferences.json"))
    } else {
        None
    }
}

pub fn load_saved_audio_preferences() -> Option<SavedAudioPreferences> {
    let path = get_audio_preferences_path()?;
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str::<SavedAudioPreferences>(&data).ok()
}

pub fn save_audio_preferences(prefs: &SavedAudioPreferences) {
    if let Some(path) = get_audio_preferences_path() {
        if let Ok(json) = serde_json::to_string_pretty(prefs) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[allow(dead_code)]
pub enum ActiveBackend {
    Cpal(Stream),
    #[cfg(target_os = "windows")]
    Asio {
        driver: Arc<asio_sys::Driver>,
        callback_id: asio_sys::CallbackId,
        is_running: Arc<AtomicBool>,
    },
}

impl Drop for ActiveBackend {
    fn drop(&mut self) {
        match self {
            ActiveBackend::Cpal(_) => {}
            #[cfg(target_os = "windows")]
            ActiveBackend::Asio { driver, callback_id, is_running } => {
                is_running.store(false, Ordering::Relaxed);
                driver.remove_callback(*callback_id);
                let _ = driver.stop();
                let _ = driver.dispose_buffers();
            }
        }
    }
}

struct StreamHolder(Option<ActiveBackend>);
unsafe impl Send for StreamHolder {}
unsafe impl Sync for StreamHolder {}

pub struct AudioEngine {
    stream: Arc<Mutex<StreamHolder>>,
    pub current_host: Arc<Mutex<String>>,
    pub target_host: Arc<Mutex<Option<String>>>,
    pub current_device_name: Arc<Mutex<String>>,
    pub target_device_name: Arc<Mutex<Option<String>>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub target_sample_rate: Arc<Mutex<Option<u32>>>,
    pub buffer_size: Arc<Mutex<u32>>,
    pub target_buffer_size: Arc<Mutex<Option<u32>>>,
    pub channels: Arc<Mutex<usize>>,
    pub selected_channel_pair: Arc<AtomicUsize>,
    pub is_running: Arc<AtomicBool>,
    pub device_invalidated: Arc<AtomicBool>,
    command_consumer: Arc<Mutex<Option<Consumer<AudioEngineCommand>>>>,
    shared_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_patch: Arc<AtomicI32>,
    shared_master_gain: Arc<Mutex<f32>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    watchdog_started: Arc<AtomicBool>,
    ui_event_thread_started: Arc<AtomicBool>,
    ui_event_producer: Arc<Mutex<Option<Producer<SequencerUiEvent>>>>,
    ui_event_consumer: Arc<Mutex<Option<Consumer<SequencerUiEvent>>>>,
    // Fase 3: Pad Sampler & Drum Kit
    pub shared_pad_samples: Arc<Mutex<[Option<Arc<PadSampleData>>; 8]>>,
    pub shared_pad_modes: Arc<Mutex<[PadPlaybackMode; 8]>>,
    pub shared_pad_gains: Arc<Mutex<[f32; 8]>>,
    pub shared_pad_engine_mode: Arc<Mutex<PadEngineMode>>,
    pub shared_drum_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    pub shared_drum_patch: Arc<AtomicI32>,
    pub shared_drum_pad_notes: Arc<Mutex<[i32; 8]>>,
    // Fase 4: FX Rack & Knobs
    pub shared_fx_rack: Arc<Mutex<AudioFxRack>>,
    pub shared_fx_routing: Arc<Mutex<FxRouting>>,
    // Fase 5: Step Sequencer (Única fuente de verdad en render state, sincronizada a shared para lecturas)
    pub shared_sequencer: Arc<Mutex<StepSequencer>>,
    // Fase 9: Arpeggiator (Sincronizado a shared para lecturas de estado)
    pub shared_arpeggiator: Arc<Mutex<Arpeggiator>>,
    #[cfg(target_os = "windows")]
    pub standby_asio_driver: Arc<Mutex<Option<Arc<asio_sys::Driver>>>>,
}

unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

fn load_factory_pad_samples(target_sample_rate: u32) -> [Option<Arc<PadSampleData>>; 8] {
    let mut slots: [Option<Arc<PadSampleData>>; 8] = std::array::from_fn(|_| None);
    let sample_dir = crate::soundfont::get_samples_dir();
    if !sample_dir.exists() {
        return slots;
    }

    let sample_files = [
        "01_Kick.wav",
        "02_Snare.wav",
        "03_Clap.wav",
        "04_Closed_Hat.wav",
        "05_Open_Hat.wav",
        "06_Low_Tom.wav",
        "07_High_Tom.wav",
        "08_Perc.wav",
    ];

    for (i, filename) in sample_files.iter().enumerate() {
        let p = sample_dir.join(filename);
        if p.exists() {
            if let Ok(data) = PadSampleData::from_file(&p, target_sample_rate) {
                slots[i] = Some(Arc::new(data));
            }
        }
    }
    slots
}

fn load_default_drum_soundfont() -> Option<Arc<SoundFont>> {
    let dir = crate::soundfont::get_soundfonts_dir();
    let rx7_path = dir.join("Yamaha_RX7_Drums.sf2");
    if rx7_path.exists() {
        if let Ok(mut file) = std::fs::File::open(&rx7_path) {
            if let Ok(sf) = SoundFont::new(&mut file) {
                return Some(Arc::new(sf));
            }
        }
    }
    None
}

struct Voice {
    note: u8,
    phase: f32,
    phase_inc: f32,
    velocity: f32,
    active: bool,
    release: bool,
    envelope: f32,
}

struct AudioRenderState {
    voices: [Voice; 16],
    master_gain: f32,
    pitch_bend_ratio: f32,
    sample_rate_f: f32,
    channels: usize,
    command_consumer: Arc<Mutex<Option<Consumer<AudioEngineCommand>>>>,
    synthesizer: Option<Synthesizer>,
    shared_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_patch: Arc<AtomicI32>,
    shared_master_gain: Arc<Mutex<f32>>,
    left_buf: Vec<f32>,
    right_buf: Vec<f32>,
    // Fase 3: Pad Sampler y Drum Kit
    pad_sampler: PadSampler,
    pad_engine_mode: PadEngineMode,
    drum_synthesizers: [Option<Synthesizer>; 8],
    pad_gains: [f32; 8],
    shared_pad_samples: Arc<Mutex<[Option<Arc<PadSampleData>>; 8]>>,
    shared_pad_modes: Arc<Mutex<[PadPlaybackMode; 8]>>,
    shared_pad_gains: Arc<Mutex<[f32; 8]>>,
    shared_pad_engine_mode: Arc<Mutex<PadEngineMode>>,
    shared_drum_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_drum_patch: Arc<AtomicI32>,
    drum_pad_notes: [i32; 8],
    shared_drum_pad_notes: Arc<Mutex<[i32; 8]>>,
    drum_left_buf: Vec<f32>,
    drum_right_buf: Vec<f32>,
    pad_left_buf: Vec<f32>,
    pad_right_buf: Vec<f32>,
    // Fase 4: FX Rack
    fx_rack: AudioFxRack,
    shared_fx_rack: Arc<Mutex<AudioFxRack>>,
    fx_routing: FxRouting,
    shared_fx_routing: Arc<Mutex<FxRouting>>,
    // Fase 5: Step Sequencer
    sequencer: StepSequencer,
    shared_sequencer: Arc<Mutex<StepSequencer>>,
    ui_event_producer: Arc<Mutex<Option<Producer<SequencerUiEvent>>>>,
    // Fase 9: Arpeggiator
    arpeggiator: Arpeggiator,
    shared_arpeggiator: Arc<Mutex<Arpeggiator>>,
    selected_channel_pair: Arc<AtomicUsize>,
    // MIDI Learn: Mapeos dinámicos CC
    midi_fader_ccs: [u8; 9],
    midi_knob_ccs: [u8; 8],
    is_midi_muted: bool,
}

impl AudioRenderState {
    fn new(
        sample_rate_f: f32,
        channels: usize,
        consumer: Arc<Mutex<Option<Consumer<AudioEngineCommand>>>>,
        shared_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
        shared_patch: Arc<AtomicI32>,
        shared_master_gain: Arc<Mutex<f32>>,
        shared_pad_samples: Arc<Mutex<[Option<Arc<PadSampleData>>; 8]>>,
        shared_pad_modes: Arc<Mutex<[PadPlaybackMode; 8]>>,
        shared_pad_gains: Arc<Mutex<[f32; 8]>>,
        shared_pad_engine_mode: Arc<Mutex<PadEngineMode>>,
        shared_drum_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
        shared_drum_patch: Arc<AtomicI32>,
        shared_drum_pad_notes: Arc<Mutex<[i32; 8]>>,
        shared_fx_rack: Arc<Mutex<AudioFxRack>>,
        shared_fx_routing: Arc<Mutex<FxRouting>>,
        shared_sequencer: Arc<Mutex<StepSequencer>>,
        ui_event_producer: Arc<Mutex<Option<Producer<SequencerUiEvent>>>>,
        shared_arpeggiator: Arc<Mutex<Arpeggiator>>,
        selected_channel_pair: Arc<AtomicUsize>,
    ) -> Self {
        let mut pad_sampler = PadSampler::new(sample_rate_f);
        let drum_pad_notes = *shared_drum_pad_notes.lock().unwrap();

        // Inicializar slots de pad con el estado compartido
        if let Ok(samples_guard) = shared_pad_samples.lock() {
            for (i, opt_s) in samples_guard.iter().enumerate() {
                if let Some(s) = opt_s {
                    pad_sampler.load_sample(i, s.clone());
                }
            }
        }
        if let Ok(modes_guard) = shared_pad_modes.lock() {
            for (i, &m) in modes_guard.iter().enumerate() {
                pad_sampler.set_mode(i, m);
            }
        }
        let mut pad_gains = [64.0 / 127.0; 8];
        if let Ok(gains_guard) = shared_pad_gains.lock() {
            for (i, &g) in gains_guard.iter().enumerate() {
                let clamped = g.clamp(0.0, 1.0);
                pad_sampler.set_gain(i, clamped);
                pad_gains[i] = clamped;
            }
        }

        let pad_engine_mode = *shared_pad_engine_mode.lock().unwrap();

        // Inicializar sintetizadores de batería independientes (1 por pad) si hay SoundFont cargado
        let mut drum_synthesizers: [Option<Synthesizer>; 8] = Default::default();
        if let Ok(sf_guard) = shared_drum_soundfont.lock() {
            if let Some(sf) = sf_guard.as_ref() {
                let settings = SynthesizerSettings::new(sample_rate_f as i32);
                let patch = shared_drum_patch.load(Ordering::SeqCst);
                for slot in drum_synthesizers.iter_mut() {
                    if let Ok(mut synth) = Synthesizer::new(sf, &settings) {
                        synth.process_midi_message(9, 0xC0, patch, 0); // Canal 10
                        *slot = Some(synth);
                    }
                }
            }
        }

        let fx_rack = if let Ok(guard) = shared_fx_rack.lock() {
            guard.clone()
        } else {
            AudioFxRack::new(sample_rate_f)
        };

        let fx_routing = *shared_fx_routing.lock().unwrap();

        let sequencer = if let Ok(guard) = shared_sequencer.lock() {
            guard.clone()
        } else {
            StepSequencer::new(sample_rate_f)
        };

        Self {
            voices: std::array::from_fn(|_| Voice {
                note: 0,
                phase: 0.0,
                phase_inc: 0.0,
                velocity: 0.0,
                active: false,
                release: false,
                envelope: 0.0,
            }),
            master_gain: 100.0 / 127.0,
            pitch_bend_ratio: 1.0,
            sample_rate_f,
            channels,
            command_consumer: consumer,
            synthesizer: None,
            shared_soundfont,
            shared_patch,
            shared_master_gain,
            left_buf: vec![0.0; 4096],
            right_buf: vec![0.0; 4096],
            pad_sampler,
            pad_engine_mode,
            drum_synthesizers,
            pad_gains,
            shared_pad_samples,
            shared_pad_modes,
            shared_pad_gains,
            shared_pad_engine_mode,
            shared_drum_soundfont,
            shared_drum_patch,
            drum_pad_notes,
            shared_drum_pad_notes,
            drum_left_buf: vec![0.0; 4096],
            drum_right_buf: vec![0.0; 4096],
            pad_left_buf: vec![0.0; 4096],
            pad_right_buf: vec![0.0; 4096],
            fx_rack,
            shared_fx_rack,
            fx_routing,
            shared_fx_routing,
            sequencer,
            shared_sequencer,
            ui_event_producer,
            arpeggiator: Arpeggiator::new(sample_rate_f),
            shared_arpeggiator,
            selected_channel_pair,
            midi_fader_ccs: [38, 39, 40, 41, 42, 43, 44, 45, 46],
            midi_knob_ccs: [56, 57, 58, 59, 60, 61, 62, 63],
            is_midi_muted: false,
        }
    }

    #[inline]
    fn send_ui_event(&self, event: SequencerUiEvent) {
        if let Ok(mut prod_guard) = self.ui_event_producer.try_lock() {
            if let Some(prod) = prod_guard.as_mut() {
                let _ = prod.push(event);
            }
        }
    }

    fn sync_shared_sequencer(&self) {
        if let Ok(mut seq) = self.shared_sequencer.try_lock() {
            *seq = self.sequencer.clone();
        }
    }

    fn sync_shared_arpeggiator(&self) {
        if let Ok(mut arp) = self.shared_arpeggiator.try_lock() {
            *arp = self.arpeggiator.clone();
        }
    }

    fn process_commands(&mut self) {
        if let Ok(mut cons_guard) = self.command_consumer.try_lock() {
            if let Some(consumer) = cons_guard.as_mut() {
                while let Ok(cmd) = consumer.pop() {
                    match cmd {
                        AudioEngineCommand::SetMidiLearnCcMappings { faders, knobs } => {
                            self.midi_fader_ccs = faders;
                            self.midi_knob_ccs = knobs;
                        }
                        AudioEngineCommand::LoadSoundFont { sound_font, patch_number, bank_number } => {
                            let settings = SynthesizerSettings::new(self.sample_rate_f as i32);
                            if let Ok(mut synth) = Synthesizer::new(&sound_font, &settings) {
                                // Enviar Bank Select MSB (CC 0), LSB (CC 32) y Program Change (0xC0) en canal 0
                                synth.process_midi_message(0, 0xB0, 0, bank_number);
                                synth.process_midi_message(0, 0xB0, 32, 0);
                                synth.process_midi_message(0, 0xC0, patch_number, 0);
                                self.synthesizer = Some(synth);
                            }
                            if let Ok(mut sf_guard) = self.shared_soundfont.lock() {
                                *sf_guard = Some(sound_font);
                            }
                            self.shared_patch.store(patch_number, Ordering::SeqCst);
                        }
                        AudioEngineCommand::SelectPreset { patch_number, bank_number } => {
                            if let Some(synth) = &mut self.synthesizer {
                                // Cortar notas activas anteriores para evitar notas colgadas con parámetros mixtos
                                synth.note_off_all(false);
                                // Enviar Bank Select MSB (CC 0), LSB (CC 32) y Program Change (0xC0) en canal 0
                                synth.process_midi_message(0, 0xB0, 0, bank_number);
                                synth.process_midi_message(0, 0xB0, 32, 0);
                                synth.process_midi_message(0, 0xC0, patch_number, 0);
                            }
                            self.shared_patch.store(patch_number, Ordering::SeqCst);
                        }
                        AudioEngineCommand::NoteOn { note, velocity } => {
                            if !self.is_midi_muted {
                                if self.arpeggiator.enabled {
                                    let vel_7bit = (velocity * 127.0).round().clamp(1.0, 127.0) as u8;
                                    self.arpeggiator.handle_note_on(note, vel_7bit);
                                    self.sync_shared_arpeggiator();
                                } else if let Some(synth) = &mut self.synthesizer {
                                    let vel_7bit = (velocity * 127.0).round().clamp(1.0, 127.0) as i32;
                                    synth.note_on(0, note as i32, vel_7bit);
                                } else {
                                    let mut slot_idx = 0;
                                    for (i, v) in self.voices.iter().enumerate() {
                                        if !v.active {
                                            slot_idx = i;
                                            break;
                                        }
                                    }

                                    let target_voice = &mut self.voices[slot_idx];
                                    target_voice.note = note;
                                    target_voice.velocity = velocity;
                                    target_voice.active = true;
                                    target_voice.release = false;
                                    target_voice.phase = 0.0;
                                    target_voice.envelope = 0.0;

                                    let freq = 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0);
                                    target_voice.phase_inc = freq / self.sample_rate_f;
                                }
                            }
                        }
                        AudioEngineCommand::NoteOff { note } => {
                            if self.arpeggiator.enabled {
                                self.arpeggiator.handle_note_off(note);
                                self.sync_shared_arpeggiator();
                            } else if let Some(synth) = &mut self.synthesizer {
                                synth.note_off(0, note as i32);
                            } else {
                                for v in self.voices.iter_mut() {
                                    if v.active && v.note == note {
                                        v.release = true;
                                    }
                                }
                            }
                        }
                        AudioEngineCommand::SetMasterGain(gain) => {
                            self.master_gain = gain.clamp(0.0, 1.0);
                            if let Ok(mut g_guard) = self.shared_master_gain.lock() {
                                *g_guard = self.master_gain;
                            }
                        }
                        AudioEngineCommand::ControlChange { controller, value } => {
                            // 1. Master Volume (Fader 9 / CC configurado / CC 46 / CC 7)
                            if controller == self.midi_fader_ccs[8] || controller == 7 || controller == 46 {
                                self.master_gain = value.clamp(0.0, 1.0);
                                if let Ok(mut g_guard) = self.shared_master_gain.try_lock() {
                                    *g_guard = self.master_gain;
                                }
                            }

                            // 2. Knobs K1..K8 (CC dinámico configurado o preset LX49+ / GM)
                            let mut knob_idx = None;
                            for (i, &cc) in self.midi_knob_ccs.iter().enumerate() {
                                if cc == controller {
                                    knob_idx = Some(i);
                                    break;
                                }
                            }
                            if knob_idx.is_none() {
                                knob_idx = match controller {
                                    56..=63 => Some((controller - 56) as usize),
                                    74 => Some(0),
                                    71 => Some(1),
                                    5 => Some(2),
                                    84 => Some(3),
                                    78 => Some(4),
                                    76 => Some(5),
                                    77 => Some(6),
                                    10 => Some(7),
                                    _ => None,
                                };
                            }
                            if let Some(idx) = knob_idx {
                                self.fx_rack.set_knob_value(idx, value);
                                if let Ok(mut fx) = self.shared_fx_rack.try_lock() {
                                    fx.set_knob_value(idx, value);
                                }
                            }

                            // 3. Faders de Pads F1..F8 (CC dinámico configurado o preset LX49+ / GM)
                            let mut pad_fader_idx = None;
                            for (i, &cc) in self.midi_fader_ccs[..8].iter().enumerate() {
                                if cc == controller {
                                    pad_fader_idx = Some(i);
                                    break;
                                }
                            }
                            if pad_fader_idx.is_none() {
                                pad_fader_idx = match controller {
                                    38..=45 => Some((controller - 38) as usize),
                                    73 => Some(0),
                                    75 => Some(1),
                                    72 => Some(2),
                                    91 => Some(3),
                                    92 => Some(4),
                                    93 => Some(5),
                                    94 => Some(6),
                                    95 => Some(7),
                                    _ => None,
                                };
                            }
                            if let Some(idx) = pad_fader_idx {
                                let pad_gain = value.clamp(0.0, 1.0);
                                self.pad_gains[idx] = pad_gain;
                                self.pad_sampler.set_gain(idx, pad_gain);
                                if let Ok(mut gains) = self.shared_pad_gains.try_lock() {
                                    gains[idx] = pad_gain;
                                }
                            }

                            // 4. Sintetizador SoundFont Melódico
                            if let Some(synth) = &mut self.synthesizer {
                                let val_7bit = (value * 127.0).round().clamp(0.0, 127.0) as i32;
                                synth.process_midi_message(0, 0xB0, controller as i32, val_7bit);
                            }
                        }
                        AudioEngineCommand::PitchBend { normalized } => {
                            let semitones = (normalized as f32 / 8192.0) * 2.0;
                            self.pitch_bend_ratio = 2.0f32.powf(semitones / 12.0);

                            if let Some(synth) = &mut self.synthesizer {
                                let raw = (normalized as i32 + 8192).clamp(0, 16383);
                                let lsb = raw & 0x7F;
                                let msb = (raw >> 7) & 0x7F;
                                synth.process_midi_message(0, 0xE0, lsb, msb);
                            }
                        }
                        // --- FASE 3: COMANDOS DE PADS ---
                        AudioEngineCommand::PadTrigger { pad_index, velocity: _ } => {
                            // Full Level (127): máxima pegada uniforme tanto al tocar el pad en vivo como al reproducirse el loop
                            let full_velocity_f = 1.0f32;
                            let vel_byte = 127u8;
                            let vel_7bit = 127i32;

                            // Si el secuenciador está en Play + Record (Overdub), grabar en vivo con cuantización y anti-flam
                            if self.sequencer.is_playing && self.sequencer.is_recording && pad_index < 8 {
                                self.sequencer.save_undo_state();
                                self.sequencer.record_live_hit(pad_index, vel_byte);
                                self.sync_shared_sequencer();
                                self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                            }

                            if !self.is_midi_muted {
                                match self.pad_engine_mode {
                                    PadEngineMode::WavSampler => {
                                        self.pad_sampler.trigger(pad_index, full_velocity_f);
                                    }
                                    PadEngineMode::DrumKitSf2 => {
                                        if pad_index < 8 {
                                            if let Some(synth) = &mut self.drum_synthesizers[pad_index] {
                                                let note = self.drum_pad_notes[pad_index];
                                                synth.note_on(9, note, vel_7bit);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        AudioEngineCommand::PadRelease { pad_index } => {
                            match self.pad_engine_mode {
                                PadEngineMode::WavSampler => {
                                    self.pad_sampler.release(pad_index);
                                }
                                PadEngineMode::DrumKitSf2 => {
                                    if pad_index < 8 {
                                        if let Some(synth) = &mut self.drum_synthesizers[pad_index] {
                                            let note = self.drum_pad_notes[pad_index];
                                            synth.note_off(9, note);
                                        }
                                    }
                                }
                            }
                        }
                        AudioEngineCommand::LoadPadSample { pad_index, sample } => {
                            self.pad_sampler.load_sample(pad_index, sample.clone());
                            if let Ok(mut slots) = self.shared_pad_samples.lock() {
                                if pad_index < 8 {
                                    slots[pad_index] = Some(sample);
                                }
                            }
                        }
                        AudioEngineCommand::ClearPadSample { pad_index } => {
                            self.pad_sampler.clear_sample(pad_index);
                            if let Ok(mut slots) = self.shared_pad_samples.lock() {
                                if pad_index < 8 {
                                    slots[pad_index] = None;
                                }
                            }
                        }
                        AudioEngineCommand::SetPadMode { pad_index, mode } => {
                            self.pad_sampler.set_mode(pad_index, mode);
                            if let Ok(mut modes) = self.shared_pad_modes.lock() {
                                if pad_index < 8 {
                                    modes[pad_index] = mode;
                                }
                            }
                        }
                        AudioEngineCommand::SetPadGain { pad_index, gain } => {
                            let clamped_gain = gain.clamp(0.0, 1.0);
                            if pad_index < 8 {
                                self.pad_gains[pad_index] = clamped_gain;
                                self.pad_sampler.set_gain(pad_index, clamped_gain);
                            }
                            if let Ok(mut gains) = self.shared_pad_gains.lock() {
                                if pad_index < 8 {
                                    gains[pad_index] = clamped_gain;
                                }
                            }
                        }
                        AudioEngineCommand::SetPadEngineMode { mode } => {
                            self.pad_engine_mode = mode;
                            if let Ok(mut m) = self.shared_pad_engine_mode.lock() {
                                *m = mode;
                            }
                        }
                        AudioEngineCommand::SetDrumPadNote { pad_index, note } => {
                            if pad_index < 8 {
                                self.drum_pad_notes[pad_index] = note;
                                if let Ok(mut notes) = self.shared_drum_pad_notes.lock() {
                                    notes[pad_index] = note;
                                }
                            }
                        }
                        AudioEngineCommand::LoadDrumKitSoundFont { sound_font, patch_number } => {
                            let settings = SynthesizerSettings::new(self.sample_rate_f as i32);
                            for slot in self.drum_synthesizers.iter_mut() {
                                if let Ok(mut synth) = Synthesizer::new(&sound_font, &settings) {
                                    synth.process_midi_message(9, 0xC0, patch_number, 0);
                                    *slot = Some(synth);
                                }
                            }
                            if let Ok(mut sf_guard) = self.shared_drum_soundfont.lock() {
                                *sf_guard = Some(sound_font);
                            }
                            self.shared_drum_patch.store(patch_number, Ordering::SeqCst);
                        }
                        // --- FASE 4: COMANDOS DE KNOBS Y FX ---
                        AudioEngineCommand::SetKnobValue { knob_index, value } => {
                            self.fx_rack.set_knob_value(knob_index, value);
                            if let Ok(mut fx) = self.shared_fx_rack.try_lock() {
                                fx.set_knob_value(knob_index, value);
                            }
                        }
                        AudioEngineCommand::SetKnobMapping { knob_index, target } => {
                            self.fx_rack.set_knob_mapping(knob_index, target);
                            if let Ok(mut fx) = self.shared_fx_rack.try_lock() {
                                fx.set_knob_mapping(knob_index, target);
                            }
                        }
                        AudioEngineCommand::SetFxRouting(routing) => {
                            self.fx_routing = routing;
                            if let Ok(mut r) = self.shared_fx_routing.try_lock() {
                                *r = routing;
                            }
                        }
                        // --- FASE 5: COMANDOS DEL SECUENCIADOR ---
                        AudioEngineCommand::SequencerPlay => {
                            self.sequencer.play();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerPause => {
                            self.sequencer.pause();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerStop => {
                            self.sequencer.stop();
                            self.sync_shared_sequencer();
                            let (_b, max_beats) = self.sequencer.get_current_beat();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                            self.send_ui_event(SequencerUiEvent::Step(SequencerStepEvent {
                                step: 0,
                                is_playing: false,
                                bpm: self.sequencer.bpm,
                                beat: 1,
                                max_beats,
                            }));
                        }
                        AudioEngineCommand::SequencerTogglePlay => {
                            let playing = self.sequencer.toggle_play();
                            self.sync_shared_sequencer();
                            let (beat, max_beats) = self.sequencer.get_current_beat();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                            if !playing {
                                self.send_ui_event(SequencerUiEvent::Step(SequencerStepEvent {
                                    step: self.sequencer.current_step,
                                    is_playing: false,
                                    bpm: self.sequencer.bpm,
                                    beat,
                                    max_beats,
                                }));
                            }
                        }
                        AudioEngineCommand::SequencerSetBpm(bpm) => {
                            self.sequencer.set_bpm(bpm);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerAdjustBpm(delta) => {
                            self.sequencer.adjust_bpm(delta);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerTapTempo => {
                            if let Some(_new_bpm) = self.sequencer.tap_tempo() {
                                self.sync_shared_sequencer();
                                self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                            }
                        }
                        AudioEngineCommand::SequencerSetRecord(rec) => {
                            self.sequencer.set_recording(rec);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerToggleRecord => {
                            let _ = self.sequencer.toggle_recording();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerSetStep { pad_index, step_index, velocity } => {
                            self.sequencer.set_step(pad_index, step_index, velocity);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                        }
                        AudioEngineCommand::SequencerToggleStep { pad_index, step_index } => {
                            self.sequencer.toggle_step(pad_index, step_index);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                        }
                        AudioEngineCommand::SequencerClear => {
                            self.sequencer.clear_pattern();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                        }
                        AudioEngineCommand::SequencerSetResolution(res) => {
                            self.sequencer.set_resolution(res);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerToggleClick => {
                            let _ = self.sequencer.toggle_click();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerToggleBank => {
                            let _ = self.sequencer.toggle_bank();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                            self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                        }
                        AudioEngineCommand::SequencerSetSelectedPad(pad_idx) => {
                            self.sequencer.set_selected_pad(pad_idx);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerClearSelectedTrack => {
                            self.sequencer.clear_selected_track();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                        }
                        AudioEngineCommand::SequencerClearTrack(pad_idx) => {
                            self.sequencer.clear_track(pad_idx);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                        }
                        AudioEngineCommand::SequencerExecuteAction(action) => {
                            match action {
                                SequencerAction::Play => {
                                    self.sequencer.play();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::Pause => {
                                    self.sequencer.pause();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::Stop => {
                                    self.sequencer.stop();
                                    self.sync_shared_sequencer();
                                    let (_b, max_beats) = self.sequencer.get_current_beat();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                    self.send_ui_event(SequencerUiEvent::Step(SequencerStepEvent {
                                        step: 0,
                                        is_playing: false,
                                        bpm: self.sequencer.bpm,
                                        beat: 1,
                                        max_beats,
                                    }));
                                }
                                SequencerAction::TogglePlay => {
                                    let playing = self.sequencer.toggle_play();
                                    self.sync_shared_sequencer();
                                    let (_b, max_beats) = self.sequencer.get_current_beat();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                    if !playing {
                                        self.send_ui_event(SequencerUiEvent::Step(SequencerStepEvent {
                                            step: 0,
                                            is_playing: false,
                                            bpm: self.sequencer.bpm,
                                            beat: 1,
                                            max_beats,
                                        }));
                                    }
                                }
                                SequencerAction::ToggleRecord => {
                                    let _ = self.sequencer.toggle_recording();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::ToggleClick => {
                                    let _ = self.sequencer.toggle_click();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::ToggleBank => {
                                    let _ = self.sequencer.toggle_bank();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                    self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                                }
                                SequencerAction::TapTempo => {
                                    if let Some(_new_bpm) = self.sequencer.tap_tempo() {
                                        self.sync_shared_sequencer();
                                        self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                    }
                                }
                                SequencerAction::ClearAll => {
                                    self.sequencer.clear_pattern();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                                }
                                SequencerAction::ClearTrack => {
                                    self.sequencer.clear_selected_track();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                                }
                                SequencerAction::ClearSelectedTrack => {
                                    self.sequencer.clear_selected_track();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                                }
                                SequencerAction::SetResolution(res) => {
                                    self.sequencer.set_resolution(res);
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::BpmDown => {
                                    self.sequencer.adjust_bpm(-1.0);
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::BpmUp => {
                                    self.sequencer.adjust_bpm(1.0);
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::SetBpm => {
                                    // No-op directo sin parámetro
                                }
                                SequencerAction::Requantize => {
                                    self.sequencer.requantize_pattern();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                                }
                                SequencerAction::Undo => {
                                    if self.sequencer.undo() {
                                        self.sync_shared_sequencer();
                                        self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                                    }
                                }
                                SequencerAction::CycleResolution => {
                                    self.sequencer.toggle_resolution();
                                    self.sync_shared_sequencer();
                                    self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                                }
                                SequencerAction::FxRouteKeys => {
                                    let current = self.fx_routing;
                                    let new_routing = match current {
                                        FxRouting::Keys => FxRouting::Off,
                                        FxRouting::Both => FxRouting::Pads,
                                        FxRouting::Pads => FxRouting::Both,
                                        FxRouting::Off => FxRouting::Keys,
                                    };
                                    self.fx_routing = new_routing;
                                    if let Ok(mut r) = self.shared_fx_routing.try_lock() {
                                        *r = new_routing;
                                    }
                                    self.send_ui_event(SequencerUiEvent::FxRoutingChanged(new_routing));
                                }
                                SequencerAction::FxRoutePads => {
                                    let current = self.fx_routing;
                                    let new_routing = match current {
                                        FxRouting::Pads => FxRouting::Off,
                                        FxRouting::Both => FxRouting::Keys,
                                        FxRouting::Keys => FxRouting::Both,
                                        FxRouting::Off => FxRouting::Pads,
                                    };
                                    self.fx_routing = new_routing;
                                    if let Ok(mut r) = self.shared_fx_routing.try_lock() {
                                        *r = new_routing;
                                    }
                                    self.send_ui_event(SequencerUiEvent::FxRoutingChanged(new_routing));
                                }
                                SequencerAction::AdvancePage | SequencerAction::CalibrateLatency | SequencerAction::ToggleWorkstationMode | SequencerAction::ToggleVisualMetronome => {
                                    // No-ops para hilo de audio / notificados directamente a UI
                                }
                            }
                        }
                        AudioEngineCommand::SequencerSetQuantizeMode(mode) => {
                            self.sequencer.set_quantize_mode(mode);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerSetQuantizeStrength(strength) => {
                            self.sequencer.set_quantize_strength(strength);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerRequantize => {
                            self.sequencer.requantize_pattern();
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                        }
                        AudioEngineCommand::SequencerSetLatencyCompensation(ticks) => {
                            self.sequencer.set_latency_compensation_ticks(ticks);
                            self.sync_shared_sequencer();
                            self.send_ui_event(SequencerUiEvent::StateChanged(self.sequencer.get_state_info()));
                        }
                        AudioEngineCommand::SequencerUndo => {
                            if self.sequencer.undo() {
                                self.sync_shared_sequencer();
                                self.send_ui_event(SequencerUiEvent::PatternUpdated(self.sequencer.get_active_pattern()));
                            }
                        }
                        AudioEngineCommand::SetMidiMute(muted) => {
                            self.is_midi_muted = muted;
                        }
                        AudioEngineCommand::SetArpEnabled(enabled) => {
                            if let Some(killed_note) = self.arpeggiator.set_enabled(enabled) {
                                if let Some(synth) = &mut self.synthesizer {
                                    synth.note_off(0, killed_note as i32);
                                }
                            }
                            self.sync_shared_arpeggiator();
                        }
                        AudioEngineCommand::SetArpHold(hold) => {
                            self.arpeggiator.set_hold(hold);
                            self.sync_shared_arpeggiator();
                        }
                        AudioEngineCommand::SetArpMode(mode) => {
                            self.arpeggiator.set_mode(mode);
                            self.sync_shared_arpeggiator();
                        }
                        AudioEngineCommand::SetArpRate(rate) => {
                            self.arpeggiator.set_rate(rate);
                            self.sync_shared_arpeggiator();
                        }
                        AudioEngineCommand::SetArpOctaves(octaves) => {
                            self.arpeggiator.set_octaves(octaves);
                            self.sync_shared_arpeggiator();
                        }
                    }
                }
            }
        }
    }

    fn next_sample(&mut self) -> f32 {
        let mut sample = 0.0f32;
        let release_step = 1.0 / (0.22 * self.sample_rate_f);
        let attack_step = 1.0 / (0.005 * self.sample_rate_f);

        for v in self.voices.iter_mut() {
            if !v.active {
                continue;
            }

            if !v.release {
                if v.envelope < 1.0 {
                    v.envelope = (v.envelope + attack_step).min(1.0);
                }
            } else {
                v.envelope -= release_step;
                if v.envelope <= 0.0 {
                    v.envelope = 0.0;
                    v.active = false;
                }
            }

            let p = v.phase * std::f32::consts::TAU;
            let wave = p.sin() * 0.65
                + (p * 2.0).sin() * 0.22
                + (p * 3.0).sin() * 0.13;

            let vel_scaled = 0.4 + 0.6 * v.velocity;
            sample += wave * vel_scaled * v.envelope * 0.45;

            v.phase += v.phase_inc * self.pitch_bend_ratio;
            if v.phase >= 1.0 {
                v.phase -= 1.0;
            }
        }
        (sample * self.master_gain).clamp(-1.0, 1.0)
    }

    fn render_internal(&mut self, num_frames: usize) {
        if self.left_buf.len() < num_frames {
            self.left_buf.resize(num_frames, 0.0);
            self.right_buf.resize(num_frames, 0.0);
        }
        if self.pad_left_buf.len() < num_frames {
            self.pad_left_buf.resize(num_frames, 0.0);
            self.pad_right_buf.resize(num_frames, 0.0);
        }

        self.pad_left_buf[..num_frames].fill(0.0);
        self.pad_right_buf[..num_frames].fill(0.0);

        // 0.5. Avanzar Arpegiador Melódico Sample-Accurate
        if self.arpeggiator.enabled {
            let bpm = self.sequencer.bpm;
            for _ in 0..num_frames {
                if let Some(event) = self.arpeggiator.advance_sample(bpm) {
                    if let Some(synth) = &mut self.synthesizer {
                        match event {
                            ArpOutputEvent::NoteTrigger { note, velocity } => {
                                synth.note_on(0, note as i32, velocity as i32);
                            }
                            ArpOutputEvent::NoteRelease { note } => {
                                synth.note_off(0, note as i32);
                            }
                        }
                    }
                }
            }
        }

        // 1. Renderizar teclado melódico en left_buf / right_buf
        if let Some(synth) = &mut self.synthesizer {
            let l_slice = &mut self.left_buf[..num_frames];
            let r_slice = &mut self.right_buf[..num_frames];
            synth.render(l_slice, r_slice);
        } else {
            for i in 0..num_frames {
                let s = self.next_sample();
                self.left_buf[i] = s;
                self.right_buf[i] = s;
            }
        }

        // 1.5. Avanzar Secuenciador Rítmico Sample-Accurate
        if self.sequencer.is_playing {
            for _ in 0..num_frames {
                if let Some((step, triggers, count)) = self.sequencer.advance_sample() {
                    for i in 0..count {
                        let (pad_idx, vel) = triggers[i];
                        match self.pad_engine_mode {
                            PadEngineMode::WavSampler => {
                                self.pad_sampler.trigger(pad_idx, vel);
                            }
                            PadEngineMode::DrumKitSf2 => {
                                if pad_idx < 8 {
                                    if let Some(drum_synth) = &mut self.drum_synthesizers[pad_idx] {
                                        let note = self.drum_pad_notes[pad_idx];
                                        let vel_7bit = (vel * 127.0).round().clamp(1.0, 127.0) as i32;
                                        drum_synth.note_on(9, note, vel_7bit);
                                    }
                                }
                            }
                        }
                    }

                    // Notificar a la UI sobre el paso activo para el metrónomo y la cuadrícula (lock-free)
                    let (beat, max_beats) = self.sequencer.get_current_beat();
                    self.send_ui_event(SequencerUiEvent::Step(SequencerStepEvent {
                        step,
                        is_playing: true,
                        bpm: self.sequencer.bpm,
                        beat,
                        max_beats,
                    }));
                }
            }
        }

        // 2. Renderizar Bus de Pads en pad_left_buf / pad_right_buf
        match self.pad_engine_mode {
            PadEngineMode::WavSampler => {
                let pl = &mut self.pad_left_buf[..num_frames];
                let pr = &mut self.pad_right_buf[..num_frames];
                self.pad_sampler.render_mix(pl, pr, num_frames);
            }
            PadEngineMode::DrumKitSf2 => {
                let pl = &mut self.pad_left_buf[..num_frames];
                let pr = &mut self.pad_right_buf[..num_frames];
                if self.drum_left_buf.len() < num_frames {
                    self.drum_left_buf.resize(num_frames, 0.0);
                    self.drum_right_buf.resize(num_frames, 0.0);
                }
                for pad_idx in 0..8 {
                    if let Some(drum_synth) = &mut self.drum_synthesizers[pad_idx] {
                        let d_l = &mut self.drum_left_buf[..num_frames];
                        let d_r = &mut self.drum_right_buf[..num_frames];
                        drum_synth.render(d_l, d_r);

                        let gain = self.pad_gains[pad_idx];
                        for i in 0..num_frames {
                            pl[i] += d_l[i] * gain;
                            pr[i] += d_r[i] * gain;
                        }
                    }
                }
            }
        }

        // 3. Procesar FX Rack según el ruteo seleccionado (Off, Keys, Pads o Both)
        match self.fx_routing {
            FxRouting::Off => {
                // Bypass total: sumar buffers secos directamente sin procesar FX (0% CPU)
                let kl = &mut self.left_buf[..num_frames];
                let kr = &mut self.right_buf[..num_frames];
                let pl = &self.pad_left_buf[..num_frames];
                let pr = &self.pad_right_buf[..num_frames];
                for i in 0..num_frames {
                    kl[i] += pl[i];
                    kr[i] += pr[i];
                }
            }
            FxRouting::Keys => {
                // Solo Teclas: procesar FX en left_buf/right_buf y luego sumar pads limpios (dry)
                let kl = &mut self.left_buf[..num_frames];
                let kr = &mut self.right_buf[..num_frames];
                for i in 0..num_frames {
                    let (proc_l, proc_r) = self.fx_rack.process_stereo_frame(kl[i], kr[i]);
                    kl[i] = proc_l;
                    kr[i] = proc_r;
                }
                let pl = &self.pad_left_buf[..num_frames];
                let pr = &self.pad_right_buf[..num_frames];
                for i in 0..num_frames {
                    kl[i] += pl[i];
                    kr[i] += pr[i];
                }
            }
            FxRouting::Pads => {
                // Solo Pads: procesar FX en pad_left_buf/pad_right_buf y luego sumar a teclas limpias (dry)
                let pl = &mut self.pad_left_buf[..num_frames];
                let pr = &mut self.pad_right_buf[..num_frames];
                for i in 0..num_frames {
                    let (proc_l, proc_r) = self.fx_rack.process_stereo_frame(pl[i], pr[i]);
                    pl[i] = proc_l;
                    pr[i] = proc_r;
                }
                let kl = &mut self.left_buf[..num_frames];
                let kr = &mut self.right_buf[..num_frames];
                for i in 0..num_frames {
                    kl[i] += pl[i];
                    kr[i] += pr[i];
                }
            }
            FxRouting::Both => {
                // Ambos: sumar pads a teclas y pasar la mezcla estéreo completa por FX
                let kl = &mut self.left_buf[..num_frames];
                let kr = &mut self.right_buf[..num_frames];
                let pl = &self.pad_left_buf[..num_frames];
                let pr = &self.pad_right_buf[..num_frames];
                for i in 0..num_frames {
                    kl[i] += pl[i];
                    kr[i] += pr[i];
                }
                for i in 0..num_frames {
                    let (proc_l, proc_r) = self.fx_rack.process_stereo_frame(kl[i], kr[i]);
                    kl[i] = proc_l;
                    kr[i] = proc_r;
                }
            }
        }

        // Render audio click metronome directly to main buffer (dry mix, 0 latency)
        if self.sequencer.is_playing && (self.sequencer.is_click_enabled || self.sequencer.click_env > 0.0001) {
            let kl = &mut self.left_buf[..num_frames];
            let kr = &mut self.right_buf[..num_frames];
            for i in 0..num_frames {
                let click = self.sequencer.render_click_sample();
                kl[i] += click;
                kr[i] += click;
            }
        }
    }

    fn render_f32(&mut self, output: &mut [f32]) {
        self.process_commands();
        let num_frames = output.len() / self.channels;
        self.render_internal(num_frames);

        let gain = self.master_gain;
        let l_slice = &self.left_buf[..num_frames];
        let r_slice = &self.right_buf[..num_frames];

        if self.channels >= 2 {
            output.fill(0.0);
            let pair = self.selected_channel_pair.load(Ordering::Relaxed);
            let left_ch = (pair * 2).min(self.channels - 2);
            let right_ch = left_ch + 1;

            for i in 0..num_frames {
                let base = i * self.channels;
                output[base + left_ch] = (l_slice[i] * gain).clamp(-1.0, 1.0);
                output[base + right_ch] = (r_slice[i] * gain).clamp(-1.0, 1.0);
            }
        } else {
            for i in 0..num_frames {
                let mono = (l_slice[i] + r_slice[i]) * 0.5 * gain;
                output[i] = mono.clamp(-1.0, 1.0);
            }
        }
    }

    fn render_i16(&mut self, output: &mut [i16]) {
        self.process_commands();
        let num_frames = output.len() / self.channels;
        self.render_internal(num_frames);

        let gain = self.master_gain;
        let l_slice = &self.left_buf[..num_frames];
        let r_slice = &self.right_buf[..num_frames];

        if self.channels >= 2 {
            output.fill(0);
            let pair = self.selected_channel_pair.load(Ordering::Relaxed);
            let left_ch = (pair * 2).min(self.channels - 2);
            let right_ch = left_ch + 1;

            for i in 0..num_frames {
                let l_val = (l_slice[i] * gain).clamp(-1.0, 1.0);
                let r_val = (r_slice[i] * gain).clamp(-1.0, 1.0);
                let base = i * self.channels;
                output[base + left_ch] = (l_val * 32767.0) as i16;
                output[base + right_ch] = (r_val * 32767.0) as i16;
            }
        } else {
            for i in 0..num_frames {
                let mono = ((l_slice[i] + r_slice[i]) * 0.5 * gain).clamp(-1.0, 1.0);
                output[i] = (mono * 32767.0) as i16;
            }
        }
    }

    fn render_u16(&mut self, output: &mut [u16]) {
        self.process_commands();
        let num_frames = output.len() / self.channels;
        self.render_internal(num_frames);

        let gain = self.master_gain;
        let l_slice = &self.left_buf[..num_frames];
        let r_slice = &self.right_buf[..num_frames];

        if self.channels >= 2 {
            output.fill(32768);
            let pair = self.selected_channel_pair.load(Ordering::Relaxed);
            let left_ch = (pair * 2).min(self.channels - 2);
            let right_ch = left_ch + 1;

            for i in 0..num_frames {
                let l_val = (l_slice[i] * gain).clamp(-1.0, 1.0);
                let r_val = (r_slice[i] * gain).clamp(-1.0, 1.0);
                let s_u16_l = ((l_val * 0.5 + 0.5) * 65535.0) as u16;
                let s_u16_r = ((r_val * 0.5 + 0.5) * 65535.0) as u16;
                let base = i * self.channels;
                output[base + left_ch] = s_u16_l;
                output[base + right_ch] = s_u16_r;
            }
        } else {
            for i in 0..num_frames {
                let mono = ((l_slice[i] + r_slice[i]) * 0.5 * gain).clamp(-1.0, 1.0);
                output[i] = ((mono * 0.5 + 0.5) * 65535.0) as u16;
            }
        }
    }
}

fn build_and_start_stream(
    device: &cpal::Device,
    supported_config: SupportedStreamConfig,
    command_consumer: Arc<Mutex<Option<Consumer<AudioEngineCommand>>>>,
    shared_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_patch: Arc<AtomicI32>,
    shared_master_gain: Arc<Mutex<f32>>,
    shared_pad_samples: Arc<Mutex<[Option<Arc<PadSampleData>>; 8]>>,
    shared_pad_modes: Arc<Mutex<[PadPlaybackMode; 8]>>,
    shared_pad_gains: Arc<Mutex<[f32; 8]>>,
    shared_pad_engine_mode: Arc<Mutex<PadEngineMode>>,
    shared_drum_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_drum_patch: Arc<AtomicI32>,
    shared_drum_pad_notes: Arc<Mutex<[i32; 8]>>,
    shared_fx_rack: Arc<Mutex<AudioFxRack>>,
    shared_fx_routing: Arc<Mutex<FxRouting>>,
    shared_sequencer: Arc<Mutex<StepSequencer>>,
    ui_event_producer: Arc<Mutex<Option<Producer<SequencerUiEvent>>>>,
    shared_arpeggiator: Arc<Mutex<Arpeggiator>>,
    device_invalidated: Arc<AtomicBool>,
    selected_channel_pair: Arc<AtomicUsize>,
    requested_buffer_size: Option<u32>,
    requested_sample_rate: Option<u32>,
) -> Result<(Stream, u32, usize), String> {
    let sample_format = supported_config.sample_format();
    let mut config: StreamConfig = supported_config.into();
    let default_sample_rate = config.sample_rate.0;

    if let Some(target_sr) = requested_sample_rate {
        let mut is_supported = false;
        if let Ok(configs) = device.supported_output_configs() {
            for c in configs {
                if target_sr >= c.min_sample_rate().0 && target_sr <= c.max_sample_rate().0 {
                    is_supported = true;
                    break;
                }
            }
        }
        if is_supported {
            config.sample_rate = cpal::SampleRate(target_sr);
        } else {
            let dev_label = device.name().unwrap_or_else(|_| "Dispositivo".to_string());
            println!(
                "[Audio Engine] Frecuencia solicitada {} Hz no soportada por '{}'. Auto-ajustando a {} Hz nativo.",
                target_sr, dev_label, default_sample_rate
            );
        }
    }

    if let Some(target_buf) = requested_buffer_size {
        config.buffer_size = cpal::BufferSize::Fixed(target_buf);
    }

    let sample_rate = config.sample_rate.0;
    let channels = config.channels as usize;

    let initial_gain = *shared_master_gain.lock().unwrap();
    let mut state = AudioRenderState::new(
        sample_rate as f32,
        channels,
        command_consumer,
        shared_soundfont.clone(),
        shared_patch.clone(),
        shared_master_gain.clone(),
        shared_pad_samples,
        shared_pad_modes,
        shared_pad_gains,
        shared_pad_engine_mode,
        shared_drum_soundfont,
        shared_drum_patch,
        shared_drum_pad_notes,
        shared_fx_rack,
        shared_fx_routing,
        shared_sequencer,
        ui_event_producer,
        shared_arpeggiator,
        selected_channel_pair,
    );
    state.master_gain = initial_gain;

    // Sincronizar todos los módulos DSP al nuevo sample rate activo
    state.fx_rack.update_sample_rate(sample_rate as f32);
    state.sequencer.update_sample_rate(sample_rate as f32);
    state.arpeggiator.update_sample_rate(sample_rate as f32);
    state.pad_sampler.update_sample_rate(sample_rate as f32);

    // Restaurar SoundFont melódico si ya había uno cargado
    if let Ok(sf_guard) = shared_soundfont.lock() {
        if let Some(sf) = sf_guard.as_ref() {
            let settings = SynthesizerSettings::new(sample_rate as i32);
            if let Ok(mut synth) = Synthesizer::new(sf, &settings) {
                let patch = shared_patch.load(Ordering::SeqCst);
                synth.process_midi_message(0, 0xC0, patch, 0);
                state.synthesizer = Some(synth);
            }
        }
    }

    let dev_inv = device_invalidated.clone();
    let err_fn = move |err: cpal::StreamError| {
        eprintln!("[Audio Watchdog] Error en cpal stream: {}", err);
        dev_inv.store(true, Ordering::SeqCst);
    };

    let stream = match sample_format {
        SampleFormat::F32 => device.build_output_stream(
            &config,
            move |output: &mut [f32], _| state.render_f32(output),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &config,
            move |output: &mut [i16], _| state.render_i16(output),
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            &config,
            move |output: &mut [u16], _| state.render_u16(output),
            err_fn,
            None,
        ),
        _ => return Err("Formato de muestra no soportado".to_string()),
    }
    .map_err(|e| format!("Error creando stream: {}", e))?;

    stream.play().map_err(|e| format!("Error reproduciendo stream: {}", e))?;

    Ok((stream, sample_rate, channels))
}

#[cfg(target_os = "windows")]
fn start_asio_streaming(
    driver: Arc<asio_sys::Driver>,
    command_consumer: Arc<Mutex<Option<Consumer<AudioEngineCommand>>>>,
    shared_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_patch: Arc<AtomicI32>,
    shared_master_gain: Arc<Mutex<f32>>,
    shared_pad_samples: Arc<Mutex<[Option<Arc<PadSampleData>>; 8]>>,
    shared_pad_modes: Arc<Mutex<[PadPlaybackMode; 8]>>,
    shared_pad_gains: Arc<Mutex<[f32; 8]>>,
    shared_pad_engine_mode: Arc<Mutex<PadEngineMode>>,
    shared_drum_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_drum_patch: Arc<AtomicI32>,
    shared_drum_pad_notes: Arc<Mutex<[i32; 8]>>,
    shared_fx_rack: Arc<Mutex<AudioFxRack>>,
    shared_fx_routing: Arc<Mutex<FxRouting>>,
    shared_sequencer: Arc<Mutex<StepSequencer>>,
    ui_event_producer: Arc<Mutex<Option<Producer<SequencerUiEvent>>>>,
    shared_arpeggiator: Arc<Mutex<Arpeggiator>>,
    selected_channel_pair: Arc<AtomicUsize>,
    requested_sample_rate: Option<u32>,
) -> Result<(ActiveBackend, u32, u32, usize), String> {
    if let Some(target_sr) = requested_sample_rate {
        if driver.can_sample_rate(target_sr as f64).unwrap_or(false) {
            let _ = driver.set_sample_rate(target_sr as f64);
        }
    }

    let sample_rate = driver.sample_rate().unwrap_or(44100.0) as u32;
    let data_type = driver
        .output_data_type()
        .map_err(|e| format!("Error obteniendo tipo de muestra ASIO: {:?}", e))?;

    let streams = driver
        .prepare_output_stream(None, 2, None)
        .map_err(|e| format!("Error preparando buffers ASIO: {:?}", e))?;
    let output_stream = streams
        .output
        .ok_or_else(|| "No se generaron buffers de salida en el driver ASIO".to_string())?;
    let buffer_size = output_stream.buffer_size as u32;

    let left_b0 = output_stream.buffer_infos[0].buffers[0] as usize;
    let left_b1 = output_stream.buffer_infos[0].buffers[1] as usize;
    let right_b0 = output_stream.buffer_infos[1].buffers[0] as usize;
    let right_b1 = output_stream.buffer_infos[1].buffers[1] as usize;

    let initial_gain = *shared_master_gain.lock().unwrap();
    let mut state = AudioRenderState::new(
        sample_rate as f32,
        2,
        command_consumer,
        shared_soundfont.clone(),
        shared_patch.clone(),
        shared_master_gain.clone(),
        shared_pad_samples,
        shared_pad_modes,
        shared_pad_gains,
        shared_pad_engine_mode,
        shared_drum_soundfont,
        shared_drum_patch,
        shared_drum_pad_notes,
        shared_fx_rack,
        shared_fx_routing,
        shared_sequencer,
        ui_event_producer,
        shared_arpeggiator,
        selected_channel_pair,
    );
    state.master_gain = initial_gain;

    state.fx_rack.update_sample_rate(sample_rate as f32);
    state.sequencer.update_sample_rate(sample_rate as f32);
    state.arpeggiator.update_sample_rate(sample_rate as f32);
    state.pad_sampler.update_sample_rate(sample_rate as f32);

    if let Ok(sf_guard) = shared_soundfont.lock() {
        if let Some(sf) = sf_guard.as_ref() {
            let settings = SynthesizerSettings::new(sample_rate as i32);
            if let Ok(mut synth) = Synthesizer::new(sf, &settings) {
                let patch = shared_patch.load(Ordering::SeqCst);
                synth.process_midi_message(0, 0xC0, patch, 0);
                state.synthesizer = Some(synth);
            }
        }
    }

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();
    let frames = buffer_size as usize;
    let mut temp_buf = vec![0.0f32; frames * 2];

    let callback_id = driver.add_callback(move |info| {
        if !is_running_clone.load(Ordering::Relaxed) {
            return;
        }
        let buf_idx = info.buffer_index;
        let left_ptr = if buf_idx == 0 {
            left_b0 as *mut std::ffi::c_void
        } else {
            left_b1 as *mut std::ffi::c_void
        };
        let right_ptr = if buf_idx == 0 {
            right_b0 as *mut std::ffi::c_void
        } else {
            right_b1 as *mut std::ffi::c_void
        };

        state.render_f32(&mut temp_buf);

        match data_type {
            asio_sys::AsioSampleType::ASIOSTInt32LSB => {
                let l = left_ptr as *mut i32;
                let r = right_ptr as *mut i32;
                for i in 0..frames {
                    unsafe {
                        *l.add(i) = (temp_buf[i * 2] * 2147483647.0) as i32;
                        *r.add(i) = (temp_buf[i * 2 + 1] * 2147483647.0) as i32;
                    }
                }
            }
            asio_sys::AsioSampleType::ASIOSTFloat32LSB => {
                let l = left_ptr as *mut f32;
                let r = right_ptr as *mut f32;
                for i in 0..frames {
                    unsafe {
                        *l.add(i) = temp_buf[i * 2];
                        *r.add(i) = temp_buf[i * 2 + 1];
                    }
                }
            }
            asio_sys::AsioSampleType::ASIOSTInt16LSB => {
                let l = left_ptr as *mut i16;
                let r = right_ptr as *mut i16;
                for i in 0..frames {
                    unsafe {
                        *l.add(i) = (temp_buf[i * 2] * 32767.0) as i16;
                        *r.add(i) = (temp_buf[i * 2 + 1] * 32767.0) as i16;
                    }
                }
            }
            asio_sys::AsioSampleType::ASIOSTInt24LSB => {
                let l = left_ptr as *mut u8;
                let r = right_ptr as *mut u8;
                for i in 0..frames {
                    let sample_l = (temp_buf[i * 2] * 8388607.0) as i32;
                    let sample_r = (temp_buf[i * 2 + 1] * 8388607.0) as i32;
                    let bytes_l = sample_l.to_le_bytes();
                    let bytes_r = sample_r.to_le_bytes();
                    unsafe {
                        *l.add(i * 3) = bytes_l[0];
                        *l.add(i * 3 + 1) = bytes_l[1];
                        *l.add(i * 3 + 2) = bytes_l[2];
                        *r.add(i * 3) = bytes_r[0];
                        *r.add(i * 3 + 1) = bytes_r[1];
                        *r.add(i * 3 + 2) = bytes_r[2];
                    }
                }
            }
            _ => {
                let l = left_ptr as *mut i32;
                let r = right_ptr as *mut i32;
                for i in 0..frames {
                    unsafe {
                        *l.add(i) = (temp_buf[i * 2] * 2147483647.0) as i32;
                        *r.add(i) = (temp_buf[i * 2 + 1] * 2147483647.0) as i32;
                    }
                }
            }
        }
    });

    driver
        .start()
        .map_err(|e| format!("Error al iniciar streaming ASIO: {:?}", e))?;

    Ok((
        ActiveBackend::Asio {
            driver,
            callback_id,
            is_running,
        },
        sample_rate,
        buffer_size,
        2,
    ))
}

#[cfg(target_os = "windows")]
fn build_and_start_asio_stream(
    driver_name: &str,
    existing_driver: Option<Arc<asio_sys::Driver>>,
    command_consumer: Arc<Mutex<Option<Consumer<AudioEngineCommand>>>>,
    shared_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_patch: Arc<AtomicI32>,
    shared_master_gain: Arc<Mutex<f32>>,
    shared_pad_samples: Arc<Mutex<[Option<Arc<PadSampleData>>; 8]>>,
    shared_pad_modes: Arc<Mutex<[PadPlaybackMode; 8]>>,
    shared_pad_gains: Arc<Mutex<[f32; 8]>>,
    shared_pad_engine_mode: Arc<Mutex<PadEngineMode>>,
    shared_drum_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_drum_patch: Arc<AtomicI32>,
    shared_drum_pad_notes: Arc<Mutex<[i32; 8]>>,
    shared_fx_rack: Arc<Mutex<AudioFxRack>>,
    shared_fx_routing: Arc<Mutex<FxRouting>>,
    shared_sequencer: Arc<Mutex<StepSequencer>>,
    ui_event_producer: Arc<Mutex<Option<Producer<SequencerUiEvent>>>>,
    shared_arpeggiator: Arc<Mutex<Arpeggiator>>,
    selected_channel_pair: Arc<AtomicUsize>,
    _requested_buffer_size: Option<u32>,
    requested_sample_rate: Option<u32>,
) -> Result<(ActiveBackend, u32, u32, usize), String> {
    let driver = match existing_driver {
        Some(d) => d,
        None => {
            let asio = asio_sys::Asio::new();
            let d = asio
                .load_driver(driver_name)
                .map_err(|e| format!("Error cargando controlador ASIO '{}': {:?}", driver_name, e))?;
            Arc::new(d)
        }
    };

    start_asio_streaming(
        driver,
        command_consumer,
        shared_soundfont,
        shared_patch,
        shared_master_gain,
        shared_pad_samples,
        shared_pad_modes,
        shared_pad_gains,
        shared_pad_engine_mode,
        shared_drum_soundfont,
        shared_drum_patch,
        shared_drum_pad_notes,
        shared_fx_rack,
        shared_fx_routing,
        shared_sequencer,
        ui_event_producer,
        shared_arpeggiator,
        selected_channel_pair,
        requested_sample_rate,
    )
}

impl AudioEngine {
    pub fn new() -> Self {
        let factory_samples = load_factory_pad_samples(48000);
        let default_drum_sf = load_default_drum_soundfont();
        let (ui_prod, ui_cons) = RingBuffer::<SequencerUiEvent>::new(128);

        let default_host = if cfg!(target_os = "windows") {
            "WASAPI".to_string()
        } else if cfg!(target_os = "macos") {
            "CoreAudio".to_string()
        } else {
            "ALSA".to_string()
        };

        Self {
            stream: Arc::new(Mutex::new(StreamHolder(None))),
            current_host: Arc::new(Mutex::new(default_host)),
            target_host: Arc::new(Mutex::new(None)),
            current_device_name: Arc::new(Mutex::new("No iniciado".to_string())),
            target_device_name: Arc::new(Mutex::new(None)),
            sample_rate: Arc::new(Mutex::new(48000)),
            target_sample_rate: Arc::new(Mutex::new(None)),
            buffer_size: Arc::new(Mutex::new(256)),
            target_buffer_size: Arc::new(Mutex::new(Some(256))),
            channels: Arc::new(Mutex::new(2)),
            selected_channel_pair: Arc::new(AtomicUsize::new(0)),
            is_running: Arc::new(AtomicBool::new(false)),
            device_invalidated: Arc::new(AtomicBool::new(false)),
            command_consumer: Arc::new(Mutex::new(None)),
            shared_soundfont: Arc::new(Mutex::new(None)),
            shared_patch: Arc::new(AtomicI32::new(0)),
            shared_master_gain: Arc::new(Mutex::new(100.0 / 127.0)),
            app_handle: Arc::new(Mutex::new(None)),
            watchdog_started: Arc::new(AtomicBool::new(false)),
            ui_event_thread_started: Arc::new(AtomicBool::new(false)),
            ui_event_producer: Arc::new(Mutex::new(Some(ui_prod))),
            ui_event_consumer: Arc::new(Mutex::new(Some(ui_cons))),
            shared_pad_samples: Arc::new(Mutex::new(factory_samples)),
            shared_pad_modes: Arc::new(Mutex::new([PadPlaybackMode::OneShot; 8])),
            shared_pad_gains: Arc::new(Mutex::new([64.0 / 127.0; 8])),
            shared_pad_engine_mode: Arc::new(Mutex::new(PadEngineMode::WavSampler)),
            shared_drum_soundfont: Arc::new(Mutex::new(default_drum_sf)),
            shared_drum_patch: Arc::new(AtomicI32::new(0)),
            shared_drum_pad_notes: Arc::new(Mutex::new([36, 38, 42, 46, 39, 45, 48, 49])),
            shared_fx_rack: Arc::new(Mutex::new(AudioFxRack::new(48000.0))),
            shared_fx_routing: Arc::new(Mutex::new(FxRouting::Off)),
            shared_sequencer: Arc::new(Mutex::new(StepSequencer::new(48000.0))),
            shared_arpeggiator: Arc::new(Mutex::new(Arpeggiator::new(48000.0))),
            #[cfg(target_os = "windows")]
            standby_asio_driver: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle.clone());
        self.spawn_ui_event_dispatcher(handle);
    }

    fn spawn_ui_event_dispatcher(&self, handle: AppHandle) {
        if self.ui_event_thread_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let consumer_mutex = self.ui_event_consumer.clone();
        std::thread::Builder::new()
            .name("ui-event-dispatcher".to_string())
            .spawn(move || {
                loop {
                    let mut processed_any = false;
                    if let Ok(mut cons_guard) = consumer_mutex.try_lock() {
                        if let Some(consumer) = cons_guard.as_mut() {
                            while let Ok(event) = consumer.pop() {
                                processed_any = true;
                                match event {
                                    SequencerUiEvent::Step(step_event) => {
                                        let _ = handle.emit("sequencer-step", step_event);
                                    }
                                    SequencerUiEvent::StateChanged(state_info) => {
                                        let _ = handle.emit("sequencer-state-changed", state_info);
                                    }
                                    SequencerUiEvent::PatternUpdated(pattern) => {
                                        let _ = handle.emit("sequencer-pattern-updated", pattern);
                                    }
                                    SequencerUiEvent::FxRoutingChanged(routing) => {
                                        let _ = handle.emit("fx-routing-changed", routing);
                                    }
                                }
                            }
                        }
                    }

                    if !processed_any {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
            })
            .expect("Error al iniciar hilo ui-event-dispatcher");
    }

    pub fn get_sequencer_state(&self) -> SequencerStateInfo {
        self.shared_sequencer.lock().unwrap().get_state_info()
    }

    pub fn get_arp_state(&self) -> ArpStateInfo {
        self.shared_arpeggiator.lock().unwrap().get_state_info()
    }

    pub fn get_pad_slots_info(&self) -> Vec<PadSlotInfo> {
        let samples_guard = self.shared_pad_samples.lock().unwrap();
        let modes_guard = self.shared_pad_modes.lock().unwrap();
        let gains_guard = self.shared_pad_gains.lock().unwrap();

        (0..8)
            .map(|i| {
                let sample_name = samples_guard[i].as_ref().map(|s| s.name.clone());
                let has_sample = samples_guard[i].is_some();
                let mode = modes_guard[i];
                let gain = gains_guard[i];
                PadSlotInfo {
                    index: i,
                    pad_number: i + 1,
                    sample_name,
                    mode,
                    gain,
                    has_sample,
                }
            })
            .collect()
    }

    pub fn get_pad_engine_mode(&self) -> PadEngineMode {
        *self.shared_pad_engine_mode.lock().unwrap()
    }

    pub fn get_knob_mappings(&self) -> Vec<KnobMappingInfo> {
        if let Ok(guard) = self.shared_fx_rack.lock() {
            guard.get_mappings_info()
        } else {
            Vec::new()
        }
    }

    #[allow(dead_code)]
    pub fn set_knob_mapping(&self, knob_idx: usize, target: KnobTarget) {
        if let Ok(mut guard) = self.shared_fx_rack.lock() {
            guard.set_knob_mapping(knob_idx, target);
        }
    }

    #[allow(dead_code)]
    pub fn set_knob_value(&self, knob_idx: usize, value: f32) {
        if let Ok(mut guard) = self.shared_fx_rack.lock() {
            guard.set_knob_value(knob_idx, value);
        }
    }

    pub fn get_fx_routing(&self) -> FxRouting {
        *self.shared_fx_routing.lock().unwrap()
    }

    pub fn set_fx_routing(&self, routing: FxRouting) {
        *self.shared_fx_routing.lock().unwrap() = routing;
    }

    pub fn list_output_devices(host_opt: Option<&str>) -> Vec<AudioDeviceInfo> {
        #[cfg(not(target_os = "windows"))]
        let _ = host_opt;
        #[cfg(target_os = "windows")]
        if host_opt == Some("ASIO") {
            let asio = asio_sys::Asio::new();
            let names = asio.driver_names();
            return names
                .into_iter()
                .enumerate()
                .map(|(idx, name)| AudioDeviceInfo {
                    id: format!("asio_{}", idx),
                    name,
                    is_default: false,
                    sample_rates: vec![44100, 48000, 88200, 96000, 176400, 192000],
                    channels: 2,
                })
                .collect();
        }

        let host = cpal::default_host();
        let default_device = host.default_output_device().and_then(|d| d.name().ok());
        let mut devices = Vec::new();

        if let Ok(dev_iter) = host.output_devices() {
            for (idx, dev) in dev_iter.enumerate() {
                if let Ok(name) = dev.name() {
                    let is_default = default_device.as_ref().map_or(false, |d| d == &name);
                    let channels = dev.default_output_config().map(|c| c.channels()).unwrap_or(2);

                    let candidates = [44100, 48000, 88200, 96000, 176400, 192000];
                    let mut supported_rates = Vec::new();
                    if let Ok(configs) = dev.supported_output_configs() {
                        let cfg_list: Vec<_> = configs.collect();
                        for &rate in &candidates {
                            for config in &cfg_list {
                                if rate >= config.min_sample_rate().0 && rate <= config.max_sample_rate().0 {
                                    if !supported_rates.contains(&rate) {
                                        supported_rates.push(rate);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    if supported_rates.is_empty() {
                        if let Ok(def_cfg) = dev.default_output_config() {
                            supported_rates.push(def_cfg.sample_rate().0);
                        } else {
                            supported_rates = vec![44100, 48000];
                        }
                    }
                    supported_rates.sort_unstable();

                    devices.push(AudioDeviceInfo {
                        id: format!("dev_{}", idx),
                        name,
                        is_default,
                        sample_rates: supported_rates,
                        channels,
                    });
                }
            }
        }
        devices
    }

    pub fn set_channel_pair(&self, pair: usize) {
        self.selected_channel_pair.store(pair, Ordering::Relaxed);
        println!("[Audio Engine] Par de canales estéreo asignado a: {}", pair);
    }

    pub fn apply_audio_config(
        &self,
        host_name: Option<String>,
        device_name: Option<String>,
        channel_pair: usize,
        buffer_size: Option<u32>,
        sample_rate: Option<u32>,
    ) -> Result<crate::state::AudioStatusInfo, String> {
        let chosen_host = host_name
            .or_else(|| self.target_host.lock().unwrap().clone())
            .unwrap_or_else(|| self.current_host.lock().unwrap().clone());

        // 1. Guardar preferencias de hardware y canal
        *self.target_host.lock().unwrap() = Some(chosen_host.clone());
        *self.target_device_name.lock().unwrap() = device_name.clone();
        *self.target_buffer_size.lock().unwrap() = buffer_size;
        *self.target_sample_rate.lock().unwrap() = sample_rate;
        self.selected_channel_pair.store(channel_pair, Ordering::Relaxed);

        // 2. Detener stream activo para liberar recursos del DAC
        {
            let mut s_guard = self.stream.lock().unwrap();
            *s_guard = StreamHolder(None);
        }
        self.is_running.store(false, Ordering::SeqCst);

        // 3. Modo ASIO nativo en Windows
        #[cfg(target_os = "windows")]
        if chosen_host == "ASIO" {
            let asio = asio_sys::Asio::new();
            let target_name = match device_name {
                Some(ref name) if !name.is_empty() => name.clone(),
                _ => {
                    let names = asio.driver_names();
                    names.into_iter().next().ok_or_else(|| "No se encontró ningún controlador ASIO instalado en el sistema".to_string())?
                }
            };

            // Reutilizar el driver en standby si es del mismo dispositivo, o liberarlo si es diferente
            let driver_to_use = {
                let mut standby = self.standby_asio_driver.lock().unwrap();
                if let Some(ref d) = *standby {
                    if d.name() == target_name {
                        Some(d.clone())
                    } else {
                        let old = standby.take();
                        drop(old);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        None
                    }
                } else {
                    None
                }
            };

            let (active_backend, effective_sr, effective_buf, channels) = build_and_start_asio_stream(
                &target_name,
                driver_to_use,
                self.command_consumer.clone(),
                self.shared_soundfont.clone(),
                self.shared_patch.clone(),
                self.shared_master_gain.clone(),
                self.shared_pad_samples.clone(),
                self.shared_pad_modes.clone(),
                self.shared_pad_gains.clone(),
                self.shared_pad_engine_mode.clone(),
                self.shared_drum_soundfont.clone(),
                self.shared_drum_patch.clone(),
                self.shared_drum_pad_notes.clone(),
                self.shared_fx_rack.clone(),
                self.shared_fx_routing.clone(),
                self.shared_sequencer.clone(),
                self.ui_event_producer.clone(),
                self.shared_arpeggiator.clone(),
                self.selected_channel_pair.clone(),
                buffer_size,
                sample_rate,
            )?;

            let driver_to_standby = match &active_backend {
                ActiveBackend::Asio { driver, .. } => Some(driver.clone()),
                _ => None,
            };
            if let Some(d) = driver_to_standby {
                *self.standby_asio_driver.lock().unwrap() = Some(d);
            }

            *self.current_host.lock().unwrap() = "ASIO".to_string();
            *self.current_device_name.lock().unwrap() = target_name.clone();
            *self.sample_rate.lock().unwrap() = effective_sr;
            *self.buffer_size.lock().unwrap() = effective_buf;
            *self.channels.lock().unwrap() = channels;
            self.is_running.store(true, Ordering::SeqCst);
            *self.stream.lock().unwrap() = StreamHolder(Some(active_backend));

            println!(
                "[Audio Engine Apply OK] Conectado exitosamente vía ASIO a '{}' ({} Hz, buffer: {}, canales: {})",
                target_name, effective_sr, effective_buf, channels
            );

            let status = crate::state::AudioStatusInfo {
                host: "ASIO".to_string(),
                device_name: target_name.clone(),
                driver: "ASIO".to_string(),
                channels,
                sample_rate: effective_sr,
                buffer_size: effective_buf,
                is_running: true,
            };

            let saved_prefs = SavedAudioPreferences {
                host: "ASIO".to_string(),
                device_name: Some(target_name),
                channel_pair,
                buffer_size: Some(effective_buf),
                sample_rate: Some(effective_sr),
            };
            save_audio_preferences(&saved_prefs);

            if let Ok(handle_guard) = self.app_handle.lock() {
                if let Some(handle) = handle_guard.as_ref() {
                    let _ = handle.emit("audio-status-changed", &status);
                }
            }

            return Ok(status);
        }

        // 4. Modo CPAL (WASAPI en Windows, CoreAudio en macOS, ALSA en Linux)
        #[cfg(target_os = "windows")]
        {
            let mut standby = self.standby_asio_driver.lock().unwrap();
            if let Some(old) = standby.take() {
                drop(old);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        let host = cpal::default_host();
        let mut resolved = None;
        if let Some(target) = &device_name {
            let target_lower = target.trim().to_lowercase();
            if let Ok(dev_iter) = host.output_devices() {
                for dev in dev_iter {
                    if let Ok(name) = dev.name() {
                        if name.trim().to_lowercase() == target_lower || name.to_lowercase().contains(&target_lower) {
                            resolved = Some((dev, name));
                            break;
                        }
                    }
                }
            }
        }

        let (device, dev_name) = match resolved {
            Some(pair) => pair,
            None => {
                let default_dev = host
                    .default_output_device()
                    .ok_or_else(|| "No se encontró ningún dispositivo de salida de audio".to_string())?;
                let def_name = default_dev
                    .name()
                    .unwrap_or_else(|_| "Dispositivo Predeterminado".to_string());
                (default_dev, def_name)
            }
        };

        let supported_config = device
            .default_output_config()
            .map_err(|e| format!("Error al obtener configuración de audio para '{}': {}", dev_name, e))?;

        let (stream, effective_sr, channels) = build_and_start_stream(
            &device,
            supported_config,
            self.command_consumer.clone(),
            self.shared_soundfont.clone(),
            self.shared_patch.clone(),
            self.shared_master_gain.clone(),
            self.shared_pad_samples.clone(),
            self.shared_pad_modes.clone(),
            self.shared_pad_gains.clone(),
            self.shared_pad_engine_mode.clone(),
            self.shared_drum_soundfont.clone(),
            self.shared_drum_patch.clone(),
            self.shared_drum_pad_notes.clone(),
            self.shared_fx_rack.clone(),
            self.shared_fx_routing.clone(),
            self.shared_sequencer.clone(),
            self.ui_event_producer.clone(),
            self.shared_arpeggiator.clone(),
            self.device_invalidated.clone(),
            self.selected_channel_pair.clone(),
            buffer_size,
            sample_rate,
        )?;

        *self.current_host.lock().unwrap() = chosen_host.clone();
        *self.current_device_name.lock().unwrap() = dev_name.clone();
        *self.sample_rate.lock().unwrap() = effective_sr;
        if let Some(buf) = buffer_size {
            *self.buffer_size.lock().unwrap() = buf;
        }
        *self.channels.lock().unwrap() = channels;
        self.is_running.store(true, Ordering::SeqCst);
        *self.stream.lock().unwrap() = StreamHolder(Some(ActiveBackend::Cpal(stream)));

        println!(
            "[Audio Engine Apply OK] Conectado exitosamente vía {} a '{}' ({} Hz, {} canales, par: {}, buffer: {:?})",
            chosen_host, dev_name, effective_sr, channels, channel_pair, buffer_size
        );

        let driver_name = format!("{:?}", host.id()).to_uppercase();
        let status = crate::state::AudioStatusInfo {
            host: chosen_host.clone(),
            device_name: dev_name.clone(),
            driver: match driver_name.as_str() {
                "WASAPI" => "WASAPI".to_string(),
                "COREAUDIO" => "CoreAudio".to_string(),
                "ALSA" => "ALSA".to_string(),
                _ => driver_name,
            },
            channels,
            sample_rate: effective_sr,
            buffer_size: *self.buffer_size.lock().unwrap(),
            is_running: true,
        };

        let saved_prefs = SavedAudioPreferences {
            host: chosen_host,
            device_name: Some(dev_name),
            channel_pair,
            buffer_size: Some(*self.buffer_size.lock().unwrap()),
            sample_rate: Some(effective_sr),
        };
        save_audio_preferences(&saved_prefs);

        if let Ok(handle_guard) = self.app_handle.lock() {
            if let Some(handle) = handle_guard.as_ref() {
                let _ = handle.emit("audio-status-changed", &status);
            }
        }

        Ok(status)
    }

    pub fn set_buffer_size(&self, buffer_size: u32) -> Result<crate::state::AudioStatusInfo, String> {
        let dev = self.target_device_name.lock().unwrap().clone();
        let pair = self.selected_channel_pair.load(Ordering::Relaxed);
        let sr = *self.target_sample_rate.lock().unwrap();
        self.apply_audio_config(None, dev, pair, Some(buffer_size), sr)
    }

    pub fn set_sample_rate(&self, sample_rate: u32) -> Result<crate::state::AudioStatusInfo, String> {
        let dev = self.target_device_name.lock().unwrap().clone();
        let pair = self.selected_channel_pair.load(Ordering::Relaxed);
        let buf = *self.target_buffer_size.lock().unwrap();
        self.apply_audio_config(None, dev, pair, buf, Some(sample_rate))
    }

    pub fn switch_output_device(&self, device_name: Option<String>) -> Result<crate::state::AudioStatusInfo, String> {
        let pair = self.selected_channel_pair.load(Ordering::Relaxed);
        let buf = *self.target_buffer_size.lock().unwrap();
        let sr = *self.target_sample_rate.lock().unwrap();
        self.apply_audio_config(None, device_name, pair, buf, sr)
    }

    pub fn suspend_audio_engine(&self) -> Result<crate::state::AudioStatusInfo, String> {
        #[cfg(target_os = "windows")]
        let is_asio = self.current_host.lock().unwrap().as_str() == "ASIO";
        #[cfg(target_os = "windows")]
        if is_asio {
            let mut s_guard = self.stream.lock().unwrap();
            if let Some(ActiveBackend::Asio { driver, .. }) = s_guard.0.as_ref() {
                *self.standby_asio_driver.lock().unwrap() = Some(driver.clone());
            }
            *s_guard = StreamHolder(None);
        } else {
            let mut s_guard = self.stream.lock().unwrap();
            *s_guard = StreamHolder(None);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut s_guard = self.stream.lock().unwrap();
            *s_guard = StreamHolder(None);
        }

        self.is_running.store(false, Ordering::SeqCst);
        println!("[Audio Engine] Audio detenido / DAC liberado para configuración (Estilo REAPER)");

        let current_host = self.current_host.lock().unwrap().clone();
        let status = crate::state::AudioStatusInfo {
            host: current_host.clone(),
            device_name: self.current_device_name.lock().unwrap().clone(),
            driver: current_host,
            channels: *self.channels.lock().unwrap(),
            sample_rate: *self.sample_rate.lock().unwrap(),
            buffer_size: *self.buffer_size.lock().unwrap(),
            is_running: false,
        };

        if let Ok(handle_guard) = self.app_handle.lock() {
            if let Some(handle) = handle_guard.as_ref() {
                let _ = handle.emit("audio-status-changed", &status);
            }
        }

        Ok(status)
    }

    pub fn resume_audio_engine(&self) -> Result<crate::state::AudioStatusInfo, String> {
        if self.is_running.load(Ordering::SeqCst) {
            let current_host = self.current_host.lock().unwrap().clone();
            return Ok(crate::state::AudioStatusInfo {
                host: current_host.clone(),
                device_name: self.current_device_name.lock().unwrap().clone(),
                driver: current_host,
                channels: *self.channels.lock().unwrap(),
                sample_rate: *self.sample_rate.lock().unwrap(),
                buffer_size: *self.buffer_size.lock().unwrap(),
                is_running: true,
            });
        }

        let host = self.current_host.lock().unwrap().clone();
        let dev = self.current_device_name.lock().unwrap().clone();
        let pair = self.selected_channel_pair.load(Ordering::Relaxed);
        let buf = *self.buffer_size.lock().unwrap();
        let sr = *self.sample_rate.lock().unwrap();

        self.apply_audio_config(Some(host), Some(dev), pair, Some(buf), Some(sr))
    }

    pub fn get_device_details(&self, host_name: &str, device_name: &str) -> Result<crate::state::AudioDeviceDetails, String> {
        #[cfg(not(target_os = "windows"))]
        let _ = host_name;
        #[cfg(target_os = "windows")]
        if host_name.eq_ignore_ascii_case("ASIO") {
            let asio = asio_sys::Asio::new();
            let driver_res = {
                let mut standby = self.standby_asio_driver.lock().unwrap();
                if let Some(ref d) = *standby {
                    if d.name() == device_name {
                        Ok(d.clone())
                    } else {
                        let old = standby.take();
                        drop(old);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        asio.load_driver(device_name).map(Arc::new)
                    }
                } else {
                    asio.load_driver(device_name).map(Arc::new)
                }
            };

            match driver_res {
                Ok(driver) => {
                    *self.standby_asio_driver.lock().unwrap() = Some(driver.clone());
                    let channels = driver.channels().map(|c| c.outs as usize).unwrap_or(0);
                    let is_available = channels > 0;
                    let current_sr = driver.sample_rate().unwrap_or(48000.0) as u32;

                    unsafe extern "C" {
                        #[link_name = "\u{1}?ASIOGetBufferSize@@YAJPEAJ000@Z"]
                        fn ASIOGetBufferSize(
                            min: *mut std::os::raw::c_long,
                            max: *mut std::os::raw::c_long,
                            pref: *mut std::os::raw::c_long,
                            gran: *mut std::os::raw::c_long,
                        ) -> i32;
                    }
                    let mut min = 0;
                    let mut max = 0;
                    let mut pref = 0;
                    let mut gran = 0;
                    let current_buf = unsafe {
                        if ASIOGetBufferSize(&mut min, &mut max, &mut pref, &mut gran) == 0 && pref > 0 {
                            pref as u32
                        } else {
                            *self.buffer_size.lock().unwrap()
                        }
                    };

                    let test_rates = [44100, 48000, 88200, 96000, 176400, 192000];
                    let mut sample_rates = Vec::new();
                    for &r in &test_rates {
                        if driver.can_sample_rate(r as f64).unwrap_or(false) {
                            sample_rates.push(r);
                        }
                    }
                    if sample_rates.is_empty() {
                        sample_rates.push(current_sr);
                    }

                    return Ok(crate::state::AudioDeviceDetails {
                        name: device_name.to_string(),
                        is_available,
                        channels,
                        sample_rates,
                        current_sample_rate: current_sr,
                        current_buffer_size: current_buf,
                    });
                }
                Err(_) => {
                    return Ok(crate::state::AudioDeviceDetails {
                        name: device_name.to_string(),
                        is_available: false,
                        channels: 0,
                        sample_rates: vec![],
                        current_sample_rate: 48000,
                        current_buffer_size: 256,
                    });
                }
            }
        }

        // Host CPAL / WASAPI / CoreAudio / ALSA
        #[cfg(target_os = "windows")]
        {
            let mut standby = self.standby_asio_driver.lock().unwrap();
            if let Some(old) = standby.take() {
                drop(old);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        let host = cpal::default_host();
        let mut target_dev = None;
        let target_lower = device_name.trim().to_lowercase();
        if let Ok(dev_iter) = host.output_devices() {
            for dev in dev_iter {
                if let Ok(name) = dev.name() {
                    if name.trim().to_lowercase() == target_lower || name.to_lowercase().contains(&target_lower) {
                        target_dev = Some((dev, name));
                        break;
                    }
                }
            }
        }

        if let Some((dev, matched_name)) = target_dev {
            let config_res = dev.default_output_config();
            let channels = config_res.as_ref().map(|c| c.channels() as usize).unwrap_or(2);
            let current_sr = config_res.as_ref().map(|c| c.sample_rate().0).unwrap_or(48000);

            let candidates = [44100, 48000, 88200, 96000, 176400, 192000];
            let mut supported_rates = Vec::new();
            if let Ok(configs) = dev.supported_output_configs() {
                let cfg_list: Vec<_> = configs.collect();
                for &rate in &candidates {
                    for config in &cfg_list {
                        if rate >= config.min_sample_rate().0 && rate <= config.max_sample_rate().0 {
                            if !supported_rates.contains(&rate) {
                                supported_rates.push(rate);
                            }
                            break;
                        }
                    }
                }
            }
            if supported_rates.is_empty() {
                supported_rates.push(current_sr);
            }
            supported_rates.sort_unstable();

            Ok(crate::state::AudioDeviceDetails {
                name: matched_name,
                is_available: true,
                channels,
                sample_rates: supported_rates,
                current_sample_rate: current_sr,
                current_buffer_size: *self.buffer_size.lock().unwrap(),
            })
        } else {
            Ok(crate::state::AudioDeviceDetails {
                name: device_name.to_string(),
                is_available: false,
                channels: 0,
                sample_rates: vec![],
                current_sample_rate: 48000,
                current_buffer_size: 256,
            })
        }
    }

    #[cfg(target_os = "windows")]
    pub fn open_asio_control_panel(&self, target_device_name: Option<String>) -> Result<crate::state::AudioStatusInfo, String> {
        let is_asio = self.current_host.lock().unwrap().as_str() == "ASIO";
        if !is_asio {
            return Err("El modo ASIO no está activo actualmente".to_string());
        }

        let req_name = target_device_name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| self.current_device_name.lock().unwrap().clone());

        // 1. Asegurar que el stream esté detenido y buffers liberados
        {
            let mut s_guard = self.stream.lock().unwrap();
            *s_guard = StreamHolder(None);
        }
        self.is_running.store(false, Ordering::SeqCst);

        // 2. Obtener el driver ASIO (de standby o cargando el solicitado)
        let driver_clone = {
            let standby = self.standby_asio_driver.lock().unwrap();
            if let Some(ref d) = *standby {
                if d.name() == req_name {
                    d.clone()
                } else {
                    drop(standby);
                    *self.standby_asio_driver.lock().unwrap() = None;
                    let asio = asio_sys::Asio::new();
                    let d = Arc::new(asio.load_driver(&req_name).map_err(|e| format!("Error cargando driver ASIO '{}': {:?}", req_name, e))?);
                    *self.standby_asio_driver.lock().unwrap() = Some(d.clone());
                    d
                }
            } else {
                drop(standby);
                let asio = asio_sys::Asio::new();
                let d = Arc::new(asio.load_driver(&req_name).map_err(|e| format!("Error cargando driver ASIO '{}': {:?}", req_name, e))?);
                *self.standby_asio_driver.lock().unwrap() = Some(d.clone());
                d
            }
        };

        println!("[ASIO Control Panel] Buffers liberados y DAC desbloqueado. Abriendo panel de control para '{}'...", req_name);

        // 3. Invocar panel de control del fabricante (Zoom F3, etc.)
        unsafe extern "C" {
            #[link_name = "?ASIOControlPanel@@YAJXZ"]
            fn ASIOControlPanel() -> i32;
        }
        let res = unsafe { ASIOControlPanel() };
        println!("[ASIO Control Panel] Invocación de panel finalizada con código: {}", res);

        // 4. Consultar valores actualizados del hardware tras el panel externo
        let new_sr = driver_clone.sample_rate().unwrap_or(*self.sample_rate.lock().unwrap() as f64) as u32;
        unsafe extern "C" {
            #[link_name = "\u{1}?ASIOGetBufferSize@@YAJPEAJ000@Z"]
            fn ASIOGetBufferSize(
                min: *mut std::os::raw::c_long,
                max: *mut std::os::raw::c_long,
                pref: *mut std::os::raw::c_long,
                gran: *mut std::os::raw::c_long,
            ) -> i32;
        }
        let mut min = 0;
        let mut max = 0;
        let mut pref = 0;
        let mut gran = 0;
        let new_buf = unsafe {
            if ASIOGetBufferSize(&mut min, &mut max, &mut pref, &mut gran) == 0 && pref > 0 {
                pref as u32
            } else {
                *self.buffer_size.lock().unwrap()
            }
        };

        *self.sample_rate.lock().unwrap() = new_sr;
        *self.buffer_size.lock().unwrap() = new_buf;
        let channels = driver_clone.channels().map(|c| c.outs as usize).unwrap_or(2);
        *self.channels.lock().unwrap() = channels;

        println!(
            "[ASIO Control Panel] Hardware sincronizado: '{}' ({} Hz, buffer: {}). Audio permanece en espera.",
            req_name, new_sr, new_buf
        );

        let status = crate::state::AudioStatusInfo {
            host: "ASIO".to_string(),
            device_name: req_name,
            driver: "ASIO".to_string(),
            channels,
            sample_rate: new_sr,
            buffer_size: new_buf,
            is_running: false,
        };

        if let Ok(handle_guard) = self.app_handle.lock() {
            if let Some(handle) = handle_guard.as_ref() {
                let _ = handle.emit("audio-status-changed", &status);
            }
        }

        Ok(status)
    }

    pub fn start(
        &mut self,
        consumer: Consumer<AudioEngineCommand>,
    ) -> Result<(), String> {
        *self.command_consumer.lock().unwrap() = Some(consumer);

        // 1. Intentar restaurar configuración preferida del usuario guardada en disco
        if let Some(prefs) = load_saved_audio_preferences() {
            #[cfg(target_os = "windows")]
            if prefs.host.eq_ignore_ascii_case("ASIO") {
                let asio = asio_sys::Asio::new();
                let names = asio.driver_names();
                let target_name = prefs.device_name.as_deref().unwrap_or("");
                let is_connected = !target_name.is_empty() && names.iter().any(|n| n.eq_ignore_ascii_case(target_name));
                if is_connected {
                    println!("[Audio Engine] Iniciando directamente con controlador ASIO preferido '{}'...", target_name);
                    match self.apply_audio_config(
                        Some("ASIO".to_string()),
                        prefs.device_name.clone(),
                        prefs.channel_pair,
                        prefs.buffer_size,
                        prefs.sample_rate,
                    ) {
                        Ok(_) => {
                            self.spawn_watchdog();
                            return Ok(());
                        }
                        Err(e) => {
                            println!("[Audio Engine] Falló inicio con ASIO preferido ({}). Recurriendo a fallback WASAPI.", e);
                        }
                    }
                } else {
                    println!("[Audio Engine] Controlador ASIO '{}' no detectado en el sistema. Recurriendo a fallback WASAPI.", target_name);
                }
            }

            if !prefs.host.eq_ignore_ascii_case("ASIO") {
                println!("[Audio Engine] Iniciando con host preferido '{}'...", prefs.host);
                match self.apply_audio_config(
                    Some(prefs.host.clone()),
                    prefs.device_name.clone(),
                    prefs.channel_pair,
                    prefs.buffer_size,
                    prefs.sample_rate,
                ) {
                    Ok(_) => {
                        self.spawn_watchdog();
                        return Ok(());
                    }
                    Err(e) => {
                        println!("[Audio Engine] Falló inicio con host '{}' ({}). Recurriendo a fallback WASAPI.", prefs.host, e);
                    }
                }
            }
        }

        // 2. Fallback por defecto: WASAPI predeterminado del sistema operativo
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No se encontró ningún dispositivo de salida de audio".to_string())?;

        let dev_name = device
            .name()
            .unwrap_or_else(|_| "Dispositivo Predeterminado".to_string());
        *self.current_device_name.lock().unwrap() = dev_name.clone();

        let supported_config = device
            .default_output_config()
            .map_err(|e| format!("Error al obtener configuración de audio: {}", e))?;

        let target_buf = *self.target_buffer_size.lock().unwrap();
        let target_sr = *self.target_sample_rate.lock().unwrap();

        let (stream, sample_rate, channels) = build_and_start_stream(
            &device,
            supported_config,
            self.command_consumer.clone(),
            self.shared_soundfont.clone(),
            self.shared_patch.clone(),
            self.shared_master_gain.clone(),
            self.shared_pad_samples.clone(),
            self.shared_pad_modes.clone(),
            self.shared_pad_gains.clone(),
            self.shared_pad_engine_mode.clone(),
            self.shared_drum_soundfont.clone(),
            self.shared_drum_patch.clone(),
            self.shared_drum_pad_notes.clone(),
            self.shared_fx_rack.clone(),
            self.shared_fx_routing.clone(),
            self.shared_sequencer.clone(),
            self.ui_event_producer.clone(),
            self.shared_arpeggiator.clone(),
            self.device_invalidated.clone(),
            self.selected_channel_pair.clone(),
            target_buf,
            target_sr,
        )?;

        *self.sample_rate.lock().unwrap() = sample_rate;
        *self.channels.lock().unwrap() = channels;
        self.is_running.store(true, Ordering::SeqCst);
        *self.stream.lock().unwrap() = StreamHolder(Some(ActiveBackend::Cpal(stream)));

        println!(
            "[Audio Engine OK] Stream iniciado en '{}' ({} Hz, {} canales)",
            dev_name, sample_rate, channels
        );

        self.spawn_watchdog();

        Ok(())
    }

    fn spawn_watchdog(&self) {
        if self.watchdog_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let current_host = self.current_host.clone();
        let stream_holder = self.stream.clone();
        let device_invalidated = self.device_invalidated.clone();
        let current_device_name = self.current_device_name.clone();
        let target_device_name = self.target_device_name.clone();
        let target_buffer_size = self.target_buffer_size.clone();
        let target_sample_rate = self.target_sample_rate.clone();
        let sample_rate = self.sample_rate.clone();
        let channels_holder = self.channels.clone();
        let selected_channel_pair = self.selected_channel_pair.clone();
        let is_running = self.is_running.clone();
        let command_consumer = self.command_consumer.clone();
        let shared_soundfont = self.shared_soundfont.clone();
        let shared_patch = self.shared_patch.clone();
        let shared_master_gain = self.shared_master_gain.clone();
        let shared_pad_samples = self.shared_pad_samples.clone();
        let shared_pad_modes = self.shared_pad_modes.clone();
        let shared_pad_gains = self.shared_pad_gains.clone();
        let shared_pad_engine_mode = self.shared_pad_engine_mode.clone();
        let shared_drum_soundfont = self.shared_drum_soundfont.clone();
        let shared_drum_patch = self.shared_drum_patch.clone();
        let shared_drum_pad_notes = self.shared_drum_pad_notes.clone();
        let shared_fx_rack = self.shared_fx_rack.clone();
        let shared_fx_routing = self.shared_fx_routing.clone();
        let shared_sequencer = self.shared_sequencer.clone();
        let shared_arpeggiator = self.shared_arpeggiator.clone();
        let ui_event_producer = self.ui_event_producer.clone();
        let app_handle = self.app_handle.clone();

        std::thread::Builder::new()
            .name("audio-watchdog".to_string())
            .spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));

                    if device_invalidated.load(Ordering::SeqCst) {
                        let host_str = current_host.lock().unwrap().clone();
                        if host_str == "ASIO" {
                            device_invalidated.store(false, Ordering::SeqCst);
                            continue;
                        }

                        stream_holder.lock().unwrap().0.take();
                        is_running.store(false, Ordering::SeqCst);
                        *current_device_name.lock().unwrap() = "Reconectando audio...".to_string();

                        if let Some(handle) = app_handle.lock().unwrap().as_ref() {
                            let _ = handle.emit("audio-status-changed", ());
                        }

                        println!("[Audio Watchdog] Dispositivo invalidado. Buscando endpoint de audio...");

                        while device_invalidated.load(Ordering::SeqCst) {
                            std::thread::sleep(std::time::Duration::from_millis(500));

                            let host = cpal::default_host();
                            let target_opt = target_device_name.lock().unwrap().clone();
                            let mut chosen = None;
                            if let Some(target) = &target_opt {
                                let target_lower = target.trim().to_lowercase();
                                if let Ok(dev_iter) = host.output_devices() {
                                    for dev in dev_iter {
                                        if let Ok(name) = dev.name() {
                                            if name.trim().to_lowercase() == target_lower || name.to_lowercase().contains(&target_lower) {
                                                chosen = Some((dev, name));
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            let (device, dev_name) = match chosen {
                                Some(pair) => pair,
                                None => {
                                    if let Some(default_dev) = host.default_output_device() {
                                        let def_name = default_dev.name().unwrap_or_else(|_| "Dispositivo Predeterminado".to_string());
                                        (default_dev, def_name)
                                    } else {
                                        continue;
                                    }
                                }
                            };

                            if let Ok(supported_config) = device.default_output_config() {
                                match build_and_start_stream(
                                    &device,
                                    supported_config,
                                    command_consumer.clone(),
                                    shared_soundfont.clone(),
                                    shared_patch.clone(),
                                    shared_master_gain.clone(),
                                    shared_pad_samples.clone(),
                                    shared_pad_modes.clone(),
                                    shared_pad_gains.clone(),
                                    shared_pad_engine_mode.clone(),
                                    shared_drum_soundfont.clone(),
                                    shared_drum_patch.clone(),
                                    shared_drum_pad_notes.clone(),
                                    shared_fx_rack.clone(),
                                    shared_fx_routing.clone(),
                                    shared_sequencer.clone(),
                                    ui_event_producer.clone(),
                                    shared_arpeggiator.clone(),
                                    device_invalidated.clone(),
                                    selected_channel_pair.clone(),
                                    *target_buffer_size.lock().unwrap(),
                                    *target_sample_rate.lock().unwrap(),
                                ) {
                                    Ok((new_stream, new_sample_rate, new_channels)) => {
                                        *stream_holder.lock().unwrap() = StreamHolder(Some(ActiveBackend::Cpal(new_stream)));
                                        *sample_rate.lock().unwrap() = new_sample_rate;
                                        *channels_holder.lock().unwrap() = new_channels;
                                        *current_device_name.lock().unwrap() = dev_name.clone();
                                        is_running.store(true, Ordering::SeqCst);
                                        device_invalidated.store(false, Ordering::SeqCst);

                                        println!(
                                            "[Audio Watchdog OK] Reconexión exitosa a '{}' ({} Hz, {} canales). Estado preservado.",
                                            dev_name, new_sample_rate, new_channels
                                        );

                                        if let Some(handle) = app_handle.lock().unwrap().as_ref() {
                                            let _ = handle.emit("audio-status-changed", ());
                                        }
                                        break;
                                    }
                                    Err(e) => {
                                        eprintln!("[Audio Watchdog] Esperando a que el dispositivo esté listo: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .expect("Error al iniciar hilo audio-watchdog");
    }
}
