use std::time::Instant;
use serde::{Deserialize, Serialize};

pub const NUM_PADS: usize = 8;
pub const NUM_STEPS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepResolution {
    Eighth,       // 1/8 (Corcheas): 16 pasos = 2 compases (30.0 / bpm seg/paso)
    Sixteenth,    // 1/16 (Semicorcheas): 16 pasos = 1 compás (15.0 / bpm seg/paso)
    ThirtySecond, // 1/32 (Fusas): 16 pasos = 0.5 compás (rolls rápidos de hi-hat)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatternBank {
    A,
    B,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerStateInfo {
    pub is_playing: bool,
    pub is_recording: bool,
    pub bpm: f32,
    pub current_step: usize,
    pub resolution: StepResolution,
    pub current_bank: PatternBank,
    pub is_click_enabled: bool,
    pub is_swing_enabled: bool,
    pub selected_pad: usize,
    pub pattern: [[u8; NUM_STEPS]; NUM_PADS],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerStepEvent {
    pub step: usize, // 0..15
    pub is_playing: bool,
    pub bpm: f32,
    pub beat: usize, // Pulso principal del compás para el Metrónomo Gigante (1..8 o 1..4)
    pub max_beats: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerBankToggleResult {
    pub current_bank: PatternBank,
    pub pattern: [[u8; NUM_STEPS]; NUM_PADS],
}

#[derive(Clone)]
pub struct StepSequencer {
    pub pattern_a: [[u8; NUM_STEPS]; NUM_PADS],
    pub pattern_b: [[u8; NUM_STEPS]; NUM_PADS],
    pub current_bank: PatternBank,
    pub bank_b_initialized: bool,
    pub is_playing: bool,
    pub is_recording: bool,
    pub bpm: f32,
    pub current_step: usize,
    pub resolution: StepResolution,
    pub is_click_enabled: bool,
    pub is_swing_enabled: bool,
    pub selected_pad: usize,
    pub sample_counter: usize,
    pub samples_per_step: usize,
    pub sample_rate: f32,
    pub need_start_trigger: bool,
    // Generador de Clic de Metrónomo Sintético (Audio Click Track)
    pub click_phase: f32,
    pub click_env: f32,
    pub click_decay: f32,
    pub click_freq: f32,
    tap_history: Vec<Instant>,
}

impl StepSequencer {
    pub fn new(sample_rate: f32) -> Self {
        let bpm = 120.0f32;
        let resolution = StepResolution::Eighth;

        // Duración base del paso (1/8 por defecto = 30.0 / bpm segundos)
        let step_duration_sec = (60.0 / bpm) / 2.0;
        let samples_per_step = (sample_rate * step_duration_sec).round() as usize;

        // Decaimiento de envolvente del clic (~18 ms)
        let click_decay = 1.0 - (1.0 / (sample_rate * 0.018).max(1.0));

        Self {
            pattern_a: [[0u8; NUM_STEPS]; NUM_PADS],
            pattern_b: [[0u8; NUM_STEPS]; NUM_PADS],
            current_bank: PatternBank::A,
            bank_b_initialized: false,
            is_playing: false,
            is_recording: false,
            bpm,
            current_step: 0,
            resolution,
            is_click_enabled: false,
            is_swing_enabled: false,
            selected_pad: 0,
            sample_counter: 0,
            samples_per_step,
            sample_rate,
            need_start_trigger: false,
            click_phase: 0.0,
            click_env: 0.0,
            click_decay,
            click_freq: 900.0,
            tap_history: Vec::with_capacity(8),
        }
    }

    #[inline]
    pub fn get_active_pattern(&self) -> &[[u8; NUM_STEPS]; NUM_PADS] {
        match self.current_bank {
            PatternBank::A => &self.pattern_a,
            PatternBank::B => &self.pattern_b,
        }
    }

    #[inline]
    pub fn get_active_pattern_mut(&mut self) -> &mut [[u8; NUM_STEPS]; NUM_PADS] {
        match self.current_bank {
            PatternBank::A => &mut self.pattern_a,
            PatternBank::B => &mut self.pattern_b,
        }
    }

    pub fn set_resolution(&mut self, resolution: StepResolution) {
        self.resolution = resolution;
        self.recalculate_samples_per_step();
    }

    #[allow(dead_code)]
    pub fn toggle_resolution(&mut self) {
        let next = match self.resolution {
            StepResolution::Eighth => StepResolution::Sixteenth,
            StepResolution::Sixteenth => StepResolution::ThirtySecond,
            StepResolution::ThirtySecond => StepResolution::Eighth,
        };
        self.set_resolution(next);
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(40.0, 260.0);
        self.recalculate_samples_per_step();
    }

    pub fn adjust_bpm(&mut self, delta: f32) {
        self.set_bpm(self.bpm + delta);
    }

    fn recalculate_samples_per_step(&mut self) {
        let step_duration_sec = match self.resolution {
            StepResolution::Eighth => (60.0 / self.bpm) / 2.0,       // Corcheas (1/8)
            StepResolution::Sixteenth => (60.0 / self.bpm) / 4.0,    // Semicorcheas (1/16)
            StepResolution::ThirtySecond => (60.0 / self.bpm) / 8.0, // Fusas (1/32)
        };
        self.samples_per_step = (self.sample_rate * step_duration_sec).round() as usize;
    }

    pub fn toggle_click(&mut self) -> bool {
        self.is_click_enabled = !self.is_click_enabled;
        self.is_click_enabled
    }

    pub fn toggle_swing(&mut self) -> bool {
        self.is_swing_enabled = !self.is_swing_enabled;
        self.is_swing_enabled
    }

    pub fn toggle_bank(&mut self) -> PatternBank {
        match self.current_bank {
            PatternBank::A => {
                // Si es la primera vez que se pasa al Banco B, clona inteligentemente el patrón de A
                if !self.bank_b_initialized {
                    self.pattern_b = self.pattern_a;
                    self.bank_b_initialized = true;
                }
                self.current_bank = PatternBank::B;
            }
            PatternBank::B => {
                self.current_bank = PatternBank::A;
            }
        }
        self.current_bank
    }

    pub fn set_selected_pad(&mut self, pad_idx: usize) {
        if pad_idx < NUM_PADS {
            self.selected_pad = pad_idx;
        }
    }

    pub fn clear_track(&mut self, pad_idx: usize) {
        if pad_idx < NUM_PADS {
            let pat = self.get_active_pattern_mut();
            pat[pad_idx].fill(0);
        }
    }

    pub fn clear_selected_track(&mut self) {
        self.clear_track(self.selected_pad);
    }

    pub fn clear_pattern(&mut self) {
        let pat = self.get_active_pattern_mut();
        for row in pat.iter_mut() {
            row.fill(0);
        }
    }

    pub fn set_step(&mut self, pad_idx: usize, step_idx: usize, velocity: u8) {
        if pad_idx < NUM_PADS && step_idx < NUM_STEPS {
            let pat = self.get_active_pattern_mut();
            pat[pad_idx][step_idx] = velocity;
        }
    }

    pub fn toggle_step(&mut self, pad_idx: usize, step_idx: usize) -> u8 {
        if pad_idx < NUM_PADS && step_idx < NUM_STEPS {
            let pat = self.get_active_pattern_mut();
            let cur = pat[pad_idx][step_idx];
            let new_val = if cur > 0 { 0 } else { 127 };
            pat[pad_idx][step_idx] = new_val;
            new_val
        } else {
            0
        }
    }

    pub fn get_current_beat(&self) -> (usize, usize) {
        match self.resolution {
            StepResolution::Eighth => ((self.current_step / 2) + 1, 8),
            StepResolution::Sixteenth => ((self.current_step / 4) + 1, 4),
            StepResolution::ThirtySecond => ((self.current_step / 8) + 1, 2),
        }
    }

    #[inline]
    fn get_step_duration_samples(&self, step: usize) -> usize {
        let base = self.samples_per_step;
        if !self.is_swing_enabled {
            return base;
        }
        // Swing clásico MPC 58%: pasos pares alargados, pasos impares acortados
        if step % 2 == 0 {
            (base as f32 * 1.16).round() as usize
        } else {
            (base as f32 * 0.84).round() as usize
        }
    }

    #[inline]
    fn trigger_click_if_beat(&mut self) {
        if !self.is_click_enabled {
            return;
        }

        let is_quarter_beat = match self.resolution {
            StepResolution::Eighth => self.current_step % 2 == 0,
            StepResolution::Sixteenth => self.current_step % 4 == 0,
            StepResolution::ThirtySecond => self.current_step % 8 == 0,
        };

        if is_quarter_beat {
            let is_downbeat = self.current_step == 0;
            self.click_freq = if is_downbeat { 1600.0 } else { 850.0 };
            self.click_env = if is_downbeat { 0.35 } else { 0.18 };
            self.click_phase = 0.0;
        }
    }

    /// Genera la muestra de audio actual del clic metrónomo si está activo
    #[inline]
    pub fn render_click_sample(&mut self) -> f32 {
        if self.click_env > 0.0001 {
            let sample = (self.click_phase * std::f32::consts::TAU).sin() * self.click_env;
            self.click_phase += self.click_freq / self.sample_rate;
            if self.click_phase >= 1.0 {
                self.click_phase -= 1.0;
            }
            self.click_env *= self.click_decay;
            sample
        } else {
            0.0
        }
    }

    pub fn tap_tempo(&mut self) -> Option<f32> {
        let now = Instant::now();
        self.tap_history.retain(|&t| now.duration_since(t).as_secs_f32() < 2.5);
        self.tap_history.push(now);

        if self.tap_history.len() >= 2 {
            let intervals: Vec<f32> = self.tap_history
                .windows(2)
                .map(|w| w[1].duration_since(w[0]).as_secs_f32())
                .collect();

            if !intervals.is_empty() {
                let avg_interval = intervals.iter().sum::<f32>() / intervals.len() as f32;
                if avg_interval > 0.05 {
                    let calculated_bpm = (60.0 / avg_interval).clamp(40.0, 260.0);
                    self.set_bpm(calculated_bpm.round());
                    return Some(self.bpm);
                }
            }
        }
        None
    }

    pub fn play(&mut self) {
        if !self.is_playing {
            self.is_playing = true;
            self.need_start_trigger = true;
        }
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.current_step = 0;
        self.sample_counter = 0;
        self.need_start_trigger = false;
        self.click_env = 0.0;
    }

    pub fn toggle_play(&mut self) -> bool {
        if self.is_playing {
            self.stop();
        } else {
            self.play();
        }
        self.is_playing
    }

    pub fn set_recording(&mut self, rec: bool) {
        self.is_recording = rec;
    }

    pub fn toggle_recording(&mut self) -> bool {
        self.is_recording = !self.is_recording;
        self.is_recording
    }

    /// Calcula el paso destino con cuantización al paso más cercano (umbral 50%)
    pub fn get_quantized_step(&self) -> usize {
        let current_duration = self.get_step_duration_samples(self.current_step);
        if self.sample_counter >= current_duration / 2 {
            (self.current_step + 1) % NUM_STEPS
        } else {
            self.current_step
        }
    }

    pub fn get_state_info(&self) -> SequencerStateInfo {
        SequencerStateInfo {
            is_playing: self.is_playing,
            is_recording: self.is_recording,
            bpm: self.bpm,
            current_step: self.current_step,
            resolution: self.resolution,
            current_bank: self.current_bank,
            is_click_enabled: self.is_click_enabled,
            is_swing_enabled: self.is_swing_enabled,
            selected_pad: self.selected_pad,
            pattern: *self.get_active_pattern(),
        }
    }

    /// Avanza un frame de audio. Retorna Some((nuevo_paso, triggers, count)) cuando ocurre un disparo.
    #[inline]
    pub fn advance_sample(&mut self) -> Option<(usize, [(usize, f32); NUM_PADS], usize)> {
        if !self.is_playing {
            return None;
        }

        // 1. Disparo instantáneo en el frame 0 exacto al presionar Play (latencia 0 ms)
        if self.need_start_trigger {
            self.need_start_trigger = false;
            self.sample_counter = 0;
            self.trigger_click_if_beat();

            let mut count = 0;
            let mut triggers = [(0, 0.0f32); NUM_PADS];
            let pattern = *self.get_active_pattern();
            for pad_idx in 0..NUM_PADS {
                let vel_byte = pattern[pad_idx][self.current_step];
                if vel_byte > 0 {
                    triggers[count] = (pad_idx, vel_byte as f32 / 127.0);
                    count += 1;
                }
            }

            return Some((self.current_step, triggers, count));
        }

        // 2. Conteo de muestras durante la duración del paso (con swing dinámico)
        self.sample_counter += 1;
        let step_duration = self.get_step_duration_samples(self.current_step);

        if self.sample_counter >= step_duration {
            self.sample_counter = 0;
            self.current_step = (self.current_step + 1) % NUM_STEPS;
            self.trigger_click_if_beat();

            let mut count = 0;
            let mut triggers = [(0, 0.0f32); NUM_PADS];
            let pattern = *self.get_active_pattern();
            for pad_idx in 0..NUM_PADS {
                let vel_byte = pattern[pad_idx][self.current_step];
                if vel_byte > 0 {
                    triggers[count] = (pad_idx, vel_byte as f32 / 127.0);
                    count += 1;
                }
            }

            Some((self.current_step, triggers, count))
        } else {
            None
        }
    }
}
