use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig, SupportedStreamConfig};
use rtrb::Consumer;
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use tauri::{AppHandle, Emitter};

use super::fx::{AudioFxRack, FxRouting, KnobMappingInfo, KnobTarget};
use super::sampler::{PadEngineMode, PadPlaybackMode, PadSampleData, PadSampler, PadSlotInfo};
use super::sequencer::{SequencerStateInfo, SequencerStepEvent, StepSequencer};
use super::{AudioDeviceInfo, AudioEngineCommand};

struct StreamHolder(Option<Stream>);
unsafe impl Send for StreamHolder {}
unsafe impl Sync for StreamHolder {}

pub struct AudioEngine {
    stream: Arc<Mutex<StreamHolder>>,
    pub current_device_name: Arc<Mutex<String>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub buffer_size: Arc<Mutex<u32>>,
    pub is_running: Arc<AtomicBool>,
    pub device_invalidated: Arc<AtomicBool>,
    command_consumer: Arc<Mutex<Option<Consumer<AudioEngineCommand>>>>,
    shared_soundfont: Arc<Mutex<Option<Arc<SoundFont>>>>,
    shared_patch: Arc<AtomicI32>,
    shared_master_gain: Arc<Mutex<f32>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    watchdog_started: Arc<AtomicBool>,
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
    // Fase 5: Step Sequencer
    pub shared_sequencer: Arc<Mutex<StepSequencer>>,
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
    shared_app_handle: Arc<Mutex<Option<AppHandle>>>,
    // MIDI Learn: Mapeos dinámicos CC
    midi_fader_ccs: [u8; 9],
    midi_knob_ccs: [u8; 8],
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
        shared_app_handle: Arc<Mutex<Option<AppHandle>>>,
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
            shared_app_handle,
            midi_fader_ccs: [38, 39, 40, 41, 42, 43, 44, 45, 46],
            midi_knob_ccs: [56, 57, 58, 59, 60, 61, 62, 63],
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
                        AudioEngineCommand::LoadSoundFont { sound_font, patch_number } => {
                            let settings = SynthesizerSettings::new(self.sample_rate_f as i32);
                            if let Ok(mut synth) = Synthesizer::new(&sound_font, &settings) {
                                synth.process_midi_message(0, 0xC0, patch_number, 0);
                                self.synthesizer = Some(synth);
                            }
                            if let Ok(mut sf_guard) = self.shared_soundfont.lock() {
                                *sf_guard = Some(sound_font);
                            }
                            self.shared_patch.store(patch_number, Ordering::SeqCst);
                        }
                        AudioEngineCommand::SelectPreset { patch_number } => {
                            if let Some(synth) = &mut self.synthesizer {
                                synth.process_midi_message(0, 0xC0, patch_number, 0);
                            }
                            self.shared_patch.store(patch_number, Ordering::SeqCst);
                        }
                        AudioEngineCommand::NoteOn { note, velocity } => {
                            if let Some(synth) = &mut self.synthesizer {
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
                        AudioEngineCommand::NoteOff { note } => {
                            if let Some(synth) = &mut self.synthesizer {
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

                            // Si el secuenciador está en Play + Record (Overdub), grabar en el paso cuantizado al 50%
                            if self.sequencer.is_playing && self.sequencer.is_recording && pad_index < 8 {
                                let target_step = self.sequencer.get_quantized_step();
                                self.sequencer.set_step(pad_index, target_step, vel_byte);
                                if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                    seq.set_step(pad_index, target_step, vel_byte);
                                }
                                if let Ok(guard) = self.shared_app_handle.try_lock() {
                                    if let Some(h) = guard.as_ref() {
                                        let _ = h.emit("sequencer-pattern-updated", *self.sequencer.get_active_pattern());
                                    }
                                }
                            }

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
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.play();
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerStop => {
                            self.sequencer.stop();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.stop();
                            }
                            let (_b, max_beats) = self.sequencer.get_current_beat();
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                    let _ = h.emit("sequencer-step", SequencerStepEvent {
                                        step: 0,
                                        is_playing: false,
                                        bpm: self.sequencer.bpm,
                                        beat: 1,
                                        max_beats,
                                    });
                                }
                            }
                        }
                        AudioEngineCommand::SequencerTogglePlay => {
                            let playing = self.sequencer.toggle_play();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                if playing { seq.play(); } else { seq.stop(); }
                            }
                            let (_b, max_beats) = self.sequencer.get_current_beat();
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                    if !playing {
                                        let _ = h.emit("sequencer-step", SequencerStepEvent {
                                            step: 0,
                                            is_playing: false,
                                            bpm: self.sequencer.bpm,
                                            beat: 1,
                                            max_beats,
                                        });
                                    }
                                }
                            }
                        }
                        AudioEngineCommand::SequencerSetBpm(bpm) => {
                            self.sequencer.set_bpm(bpm);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.set_bpm(bpm);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerAdjustBpm(delta) => {
                            self.sequencer.adjust_bpm(delta);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.adjust_bpm(delta);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerTapTempo => {
                            if let Some(new_bpm) = self.sequencer.tap_tempo() {
                                if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                    seq.set_bpm(new_bpm);
                                }
                                if let Ok(guard) = self.shared_app_handle.try_lock() {
                                    if let Some(h) = guard.as_ref() {
                                        let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                    }
                                }
                            }
                        }
                        AudioEngineCommand::SequencerSetRecord(rec) => {
                            self.sequencer.set_recording(rec);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.set_recording(rec);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerToggleRecord => {
                            let rec = self.sequencer.toggle_recording();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.set_recording(rec);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerSetStep { pad_index, step_index, velocity } => {
                            self.sequencer.set_step(pad_index, step_index, velocity);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.set_step(pad_index, step_index, velocity);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-pattern-updated", *self.sequencer.get_active_pattern());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerToggleStep { pad_index, step_index } => {
                            self.sequencer.toggle_step(pad_index, step_index);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                *seq = self.sequencer.clone();
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-pattern-updated", *self.sequencer.get_active_pattern());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerClear => {
                            self.sequencer.clear_pattern();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.clear_pattern();
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-pattern-updated", *self.sequencer.get_active_pattern());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerSetResolution(res) => {
                            self.sequencer.set_resolution(res);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.set_resolution(res);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerToggleClick => {
                            let _ = self.sequencer.toggle_click();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                let _ = seq.toggle_click();
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerToggleBank => {
                            let _ = self.sequencer.toggle_bank();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                *seq = self.sequencer.clone();
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                    let _ = h.emit("sequencer-pattern-updated", *self.sequencer.get_active_pattern());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerToggleSwing => {
                            let _ = self.sequencer.toggle_swing();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                let _ = seq.toggle_swing();
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerSetSelectedPad(pad_idx) => {
                            self.sequencer.set_selected_pad(pad_idx);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.set_selected_pad(pad_idx);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-state-changed", self.sequencer.get_state_info());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerClearSelectedTrack => {
                            self.sequencer.clear_selected_track();
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.clear_selected_track();
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-pattern-updated", *self.sequencer.get_active_pattern());
                                }
                            }
                        }
                        AudioEngineCommand::SequencerClearTrack(pad_idx) => {
                            self.sequencer.clear_track(pad_idx);
                            if let Ok(mut seq) = self.shared_sequencer.try_lock() {
                                seq.clear_track(pad_idx);
                            }
                            if let Ok(guard) = self.shared_app_handle.try_lock() {
                                if let Some(h) = guard.as_ref() {
                                    let _ = h.emit("sequencer-pattern-updated", *self.sequencer.get_active_pattern());
                                }
                            }
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

                    // Notificar a la UI sobre el paso activo para el metrónomo y la cuadrícula
                    let (beat, max_beats) = self.sequencer.get_current_beat();
                    if let Ok(guard) = self.shared_app_handle.try_lock() {
                        if let Some(h) = guard.as_ref() {
                            let _ = h.emit("sequencer-step", SequencerStepEvent {
                                step,
                                is_playing: true,
                                bpm: self.sequencer.bpm,
                                beat,
                                max_beats,
                            });
                        }
                    }
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

        if self.channels == 2 {
            for i in 0..num_frames {
                output[i * 2] = (l_slice[i] * gain).clamp(-1.0, 1.0);
                output[i * 2 + 1] = (r_slice[i] * gain).clamp(-1.0, 1.0);
            }
        } else {
            for i in 0..num_frames {
                let mono = (l_slice[i] + r_slice[i]) * 0.5 * gain;
                for ch in 0..self.channels {
                    output[i * self.channels + ch] = mono.clamp(-1.0, 1.0);
                }
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

        if self.channels == 2 {
            for i in 0..num_frames {
                let l_val = (l_slice[i] * gain).clamp(-1.0, 1.0);
                let r_val = (r_slice[i] * gain).clamp(-1.0, 1.0);
                output[i * 2] = (l_val * 32767.0) as i16;
                output[i * 2 + 1] = (r_val * 32767.0) as i16;
            }
        } else {
            for i in 0..num_frames {
                let mono = ((l_slice[i] + r_slice[i]) * 0.5 * gain).clamp(-1.0, 1.0);
                let s_i16 = (mono * 32767.0) as i16;
                for ch in 0..self.channels {
                    output[i * self.channels + ch] = s_i16;
                }
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

        if self.channels == 2 {
            for i in 0..num_frames {
                let l_val = (l_slice[i] * gain).clamp(-1.0, 1.0);
                let r_val = (r_slice[i] * gain).clamp(-1.0, 1.0);
                let s_u16_l = ((l_val * 0.5 + 0.5) * 65535.0) as u16;
                let s_u16_r = ((r_val * 0.5 + 0.5) * 65535.0) as u16;
                output[i * 2] = s_u16_l;
                output[i * 2 + 1] = s_u16_r;
            }
        } else {
            for i in 0..num_frames {
                let mono = ((l_slice[i] + r_slice[i]) * 0.5 * gain).clamp(-1.0, 1.0);
                let s_u16 = ((mono * 0.5 + 0.5) * 65535.0) as u16;
                for ch in 0..self.channels {
                    output[i * self.channels + ch] = s_u16;
                }
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
    shared_app_handle: Arc<Mutex<Option<AppHandle>>>,
    device_invalidated: Arc<AtomicBool>,
) -> Result<(Stream, u32, usize), String> {
    let sample_format = supported_config.sample_format();
    let config: StreamConfig = supported_config.into();

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
        shared_app_handle,
    );
    state.master_gain = initial_gain;

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

impl AudioEngine {
    pub fn new() -> Self {
        let factory_samples = load_factory_pad_samples(48000);
        let default_drum_sf = load_default_drum_soundfont();

        Self {
            stream: Arc::new(Mutex::new(StreamHolder(None))),
            current_device_name: Arc::new(Mutex::new("No iniciado".to_string())),
            sample_rate: Arc::new(Mutex::new(48000)),
            buffer_size: Arc::new(Mutex::new(256)),
            is_running: Arc::new(AtomicBool::new(false)),
            device_invalidated: Arc::new(AtomicBool::new(false)),
            command_consumer: Arc::new(Mutex::new(None)),
            shared_soundfont: Arc::new(Mutex::new(None)),
            shared_patch: Arc::new(AtomicI32::new(0)),
            shared_master_gain: Arc::new(Mutex::new(100.0 / 127.0)),
            app_handle: Arc::new(Mutex::new(None)),
            watchdog_started: Arc::new(AtomicBool::new(false)),
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
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }

    pub fn get_sequencer_state(&self) -> SequencerStateInfo {
        self.shared_sequencer.lock().unwrap().get_state_info()
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

    pub fn list_output_devices() -> Vec<AudioDeviceInfo> {
        let host = cpal::default_host();
        let default_device = host.default_output_device().and_then(|d| d.name().ok());
        let mut devices = Vec::new();

        if let Ok(dev_iter) = host.output_devices() {
            for (idx, dev) in dev_iter.enumerate() {
                if let Ok(name) = dev.name() {
                    let is_default = default_device.as_ref().map_or(false, |d| d == &name);
                    devices.push(AudioDeviceInfo {
                        id: format!("dev_{}", idx),
                        name,
                        is_default,
                        sample_rates: vec![44100, 48000, 96000],
                    });
                }
            }
        }
        devices
    }

    pub fn start(
        &mut self,
        consumer: Consumer<AudioEngineCommand>,
    ) -> Result<(), String> {
        *self.command_consumer.lock().unwrap() = Some(consumer);

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
            self.app_handle.clone(),
            self.device_invalidated.clone(),
        )?;

        *self.sample_rate.lock().unwrap() = sample_rate;
        self.is_running.store(true, Ordering::SeqCst);
        *self.stream.lock().unwrap() = StreamHolder(Some(stream));

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

        let stream_holder = self.stream.clone();
        let device_invalidated = self.device_invalidated.clone();
        let current_device_name = self.current_device_name.clone();
        let sample_rate = self.sample_rate.clone();
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
        let app_handle = self.app_handle.clone();

        std::thread::Builder::new()
            .name("audio-watchdog".to_string())
            .spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));

                    if device_invalidated.load(Ordering::SeqCst) {
                        stream_holder.lock().unwrap().0.take();
                        is_running.store(false, Ordering::SeqCst);
                        *current_device_name.lock().unwrap() = "Reconectando audio...".to_string();

                        if let Some(handle) = app_handle.lock().unwrap().as_ref() {
                            let _ = handle.emit("audio-status-changed", ());
                        }

                        println!("[Audio Watchdog] Dispositivo invalidado. Buscando nuevo endpoint de audio en Windows...");

                        while device_invalidated.load(Ordering::SeqCst) {
                            std::thread::sleep(std::time::Duration::from_millis(500));

                            let host = cpal::default_host();
                            if let Some(device) = host.default_output_device() {
                                let dev_name = device.name().unwrap_or_else(|_| "Dispositivo Predeterminado".to_string());
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
                                        app_handle.clone(),
                                        device_invalidated.clone(),
                                    ) {
                                        Ok((new_stream, new_sample_rate, _channels)) => {
                                            *stream_holder.lock().unwrap() = StreamHolder(Some(new_stream));
                                            *sample_rate.lock().unwrap() = new_sample_rate;
                                            *current_device_name.lock().unwrap() = dev_name.clone();
                                            is_running.store(true, Ordering::SeqCst);
                                            device_invalidated.store(false, Ordering::SeqCst);

                                            println!(
                                                "[Audio Watchdog OK] Reconexión exitosa a '{}' ({} Hz). Instrumentos y samples restaurados.",
                                                dev_name, new_sample_rate
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
                }
            })
            .expect("Error al iniciar hilo audio-watchdog");
    }
}
