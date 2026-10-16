use std::time::Instant;
use serde::{Deserialize, Serialize};

pub const NUM_PADS: usize = 8;
#[allow(dead_code)]
pub const NUM_STEPS: usize = 16;
pub const MAX_STEPS: usize = 32;
pub const TICKS_PER_BEAT: u32 = 96;
pub const BEATS_PER_BAR: u32 = 4;
pub const TICKS_PER_BAR: u32 = BEATS_PER_BAR * TICKS_PER_BEAT; // 384 ticks
pub const MAX_EVENTS_PER_TRACK: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerEvent {
    pub tick: u32,
    pub original_tick: u32,
    pub velocity: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct TrackEvents {
    pub events: [SequencerEvent; MAX_EVENTS_PER_TRACK],
    pub count: usize,
}

impl TrackEvents {
    pub const fn new() -> Self {
        Self {
            events: [SequencerEvent { tick: 0, original_tick: 0, velocity: 0 }; MAX_EVENTS_PER_TRACK],
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }

    pub fn add_or_update(&mut self, tick: u32, original_tick: u32, velocity: u8) {
        let tick = tick % TICKS_PER_BAR;
        let original_tick = original_tick % TICKS_PER_BAR;
        for i in 0..self.count {
            if self.events[i].tick == tick {
                if velocity == 0 {
                    self.remove_at(i);
                } else {
                    self.events[i].velocity = velocity;
                    self.events[i].original_tick = original_tick;
                }
                return;
            }
        }
        if velocity > 0 && self.count < MAX_EVENTS_PER_TRACK {
            self.events[self.count] = SequencerEvent { tick, original_tick, velocity };
            self.count += 1;
        }
    }

    pub fn remove_at(&mut self, index: usize) {
        if index < self.count {
            for i in index..self.count - 1 {
                self.events[i] = self.events[i + 1];
            }
            self.count -= 1;
        }
    }

    pub fn remove_at_tick(&mut self, tick: u32) -> bool {
        let tick = tick % TICKS_PER_BAR;
        for i in 0..self.count {
            if self.events[i].tick == tick {
                self.remove_at(i);
                return true;
            }
        }
        false
    }

    pub fn requantize(&mut self, mode: QuantizeMode, strength: f32) {
        if let Some(grid) = mode.grid_ticks() {
            let strength = strength.clamp(0.0, 1.0);
            for i in 0..self.count {
                let orig = self.events[i].original_tick % TICKS_PER_BAR;
                let nearest = (((orig as f32 / grid as f32).round() as u32) * grid) % TICKS_PER_BAR;
                let diff = (nearest as i32 - orig as i32 + TICKS_PER_BAR as i32 + (TICKS_PER_BAR as i32 / 2)) % (TICKS_PER_BAR as i32) - (TICKS_PER_BAR as i32 / 2);
                let shifted = orig as i32 + (diff as f32 * strength).round() as i32;
                let quantized = (shifted + TICKS_PER_BAR as i32 * 2) as u32 % TICKS_PER_BAR;
                self.events[i].tick = quantized;
            }
        } else {
            for i in 0..self.count {
                self.events[i].tick = self.events[i].original_tick;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BankPattern {
    pub tracks: [TrackEvents; NUM_PADS],
}

impl BankPattern {
    pub const fn new() -> Self {
        Self {
            tracks: [TrackEvents::new(); NUM_PADS],
        }
    }

    pub fn clear(&mut self) {
        for t in self.tracks.iter_mut() {
            t.clear();
        }
    }

    pub fn requantize(&mut self, mode: QuantizeMode, strength: f32) {
        for t in self.tracks.iter_mut() {
            t.requantize(mode, strength);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepResolution {
    Eighth,       // 1/8 (Corcheas): 8 pasos en 1 compás (48 ticks/paso)
    Sixteenth,    // 1/16 (Semicorcheas): 16 pasos en 1 compás (24 ticks/paso)
    ThirtySecond, // 1/32 (Fusas): 32 pasos en 1 compás (12 ticks/paso)
}

impl StepResolution {
    #[inline]
    pub fn ticks_per_step(&self) -> u32 {
        match self {
            StepResolution::Eighth => 48,       // 1/8 = media negra = 48 ticks
            StepResolution::Sixteenth => 24,    // 1/16 = cuarto de negra = 24 ticks
            StepResolution::ThirtySecond => 12, // 1/32 = octavo de negra = 12 ticks
        }
    }

    #[inline]
    pub fn steps_per_bar(&self) -> usize {
        match self {
            StepResolution::Eighth => 8,
            StepResolution::Sixteenth => 16,
            StepResolution::ThirtySecond => 32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatternBank {
    A,
    B,
}

/// Catálogo de acciones del secuenciador con nombre
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SequencerAction {
    Play,
    Pause,
    Stop,
    TogglePlay,
    ToggleRecord,
    ToggleClick,
    ToggleBank,
    TapTempo,
    ClearAll,
    ClearTrack,
    ClearSelectedTrack,
    SetResolution(StepResolution),
    BpmDown,
    BpmUp,
    SetBpm,
    AdvancePage,
    Undo,
    Requantize,
    CalibrateLatency,
    CycleResolution,
    FxRouteKeys,
    FxRoutePads,
    ToggleVisualMetronome,
    ToggleWorkstationMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuantizeMode {
    Off,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl QuantizeMode {
    #[inline]
    pub fn grid_ticks(&self) -> Option<u32> {
        match self {
            QuantizeMode::Off => None,
            QuantizeMode::Eighth => Some(48),
            QuantizeMode::Sixteenth => Some(24),
            QuantizeMode::ThirtySecond => Some(12),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerStateInfo {
    pub is_playing: bool,
    pub is_paused: bool,
    pub is_recording: bool,
    pub bpm: f32,
    pub current_step: usize,
    pub resolution: StepResolution,
    pub current_bank: PatternBank,
    pub is_click_enabled: bool,
    pub selected_pad: usize,
    pub quantize_mode: QuantizeMode,
    pub quantize_strength: f32,
    pub latency_compensation_ticks: i32,
    pub pattern: [[u8; MAX_STEPS]; NUM_PADS],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerStepEvent {
    pub step: usize, // 0..31
    pub is_playing: bool,
    pub bpm: f32,
    pub beat: usize, // Pulso principal del compás para el Metrónomo Gigante (1..4)
    pub max_beats: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerBankToggleResult {
    pub current_bank: PatternBank,
    pub pattern: [[u8; MAX_STEPS]; NUM_PADS],
}

#[derive(Clone)]
pub struct StepSequencer {
    pub bank_a: BankPattern,
    pub bank_b: BankPattern,
    pub current_bank: PatternBank,
    pub bank_b_initialized: bool,
    pub is_playing: bool,
    pub is_paused: bool,
    pub is_recording: bool,
    pub bpm: f32,
    pub current_tick: u32,       // 0..383
    pub current_step: usize,     // 0..31 paso según resolución activa
    pub resolution: StepResolution,
    pub is_click_enabled: bool,
    pub selected_pad: usize,
    pub quantize_mode: QuantizeMode,
    pub quantize_strength: f32,
    pub latency_compensation_ticks: i32,
    pub anti_flam_pad: Option<usize>,
    pub anti_flam_tick: u32,
    pub sample_accum: f32,
    pub samples_per_tick: f32,
    pub sample_rate: f32,
    pub need_start_trigger: bool,
    // Generador de Clic de Metrónomo Sintético (Audio Click Track)
    pub click_phase: f32,
    pub click_env: f32,
    pub click_decay: f32,
    pub click_freq: f32,
    tap_history: Vec<Instant>,
    undo_history: Vec<BankPattern>,
}

impl StepSequencer {
    pub fn new(sample_rate: f32) -> Self {
        let bpm = 120.0f32;
        let resolution = StepResolution::Sixteenth;

        // Decaimiento de envolvente del clic (~18 ms)
        let click_decay = 1.0 - (1.0 / (sample_rate * 0.018).max(1.0));

        let mut seq = Self {
            bank_a: BankPattern::new(),
            bank_b: BankPattern::new(),
            current_bank: PatternBank::A,
            bank_b_initialized: false,
            is_playing: false,
            is_paused: false,
            is_recording: false,
            bpm,
            current_tick: 0,
            current_step: 0,
            resolution,
            is_click_enabled: false,
            selected_pad: 0,
            quantize_mode: QuantizeMode::Sixteenth,
            quantize_strength: 1.0,
            latency_compensation_ticks: 0,
            anti_flam_pad: None,
            anti_flam_tick: 0,
            sample_accum: 0.0,
            samples_per_tick: 0.0,
            sample_rate,
            need_start_trigger: false,
            click_phase: 0.0,
            click_env: 0.0,
            click_decay,
            click_freq: 900.0,
            tap_history: Vec::with_capacity(8),
            undo_history: Vec::with_capacity(16),
        };
        seq.recalculate_samples_per_tick();
        seq
    }

    #[inline]
    pub fn get_active_bank(&self) -> &BankPattern {
        match self.current_bank {
            PatternBank::A => &self.bank_a,
            PatternBank::B => &self.bank_b,
        }
    }

    #[inline]
    pub fn get_active_bank_mut(&mut self) -> &mut BankPattern {
        match self.current_bank {
            PatternBank::A => &mut self.bank_a,
            PatternBank::B => &mut self.bank_b,
        }
    }

    /// Proyecta los eventos en ticks del banco activo a la cuadrícula (hasta 32 pasos según resolución)
    pub fn get_active_pattern(&self) -> [[u8; MAX_STEPS]; NUM_PADS] {
        let mut grid = [[0u8; MAX_STEPS]; NUM_PADS];
        let bank = self.get_active_bank();
        let ticks_step = self.resolution.ticks_per_step();
        let max_steps = self.resolution.steps_per_bar();

        for pad_idx in 0..NUM_PADS {
            let track = &bank.tracks[pad_idx];
            for i in 0..track.count {
                let ev = track.events[i];
                // Asignar a la celda de paso más cercana (ventana +- 50% del paso)
                let step_idx = ((ev.tick as f32 + (ticks_step as f32 / 2.0)) / ticks_step as f32).floor() as usize;
                if step_idx < max_steps && step_idx < MAX_STEPS {
                    grid[pad_idx][step_idx] = ev.velocity;
                }
            }
        }
        grid
    }

    pub fn set_resolution(&mut self, resolution: StepResolution) {
        self.resolution = resolution;
        self.update_current_step();
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
        self.bpm = bpm.clamp(40.0, 300.0);
        self.recalculate_samples_per_tick();
    }

    pub fn adjust_bpm(&mut self, delta: f32) {
        self.set_bpm(self.bpm + delta);
    }

    fn recalculate_samples_per_tick(&mut self) {
        // 96 ticks por negra -> 1 negra = (60.0 / bpm) segundos -> 1 tick = (60.0 / (bpm * 96.0)) seg
        let seconds_per_tick = 60.0 / (self.bpm * TICKS_PER_BEAT as f32);
        self.samples_per_tick = self.sample_rate * seconds_per_tick;
    }

    pub fn update_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.click_decay = 1.0 - (1.0 / (sample_rate * 0.018).max(1.0));
        self.recalculate_samples_per_tick();
    }

    pub fn toggle_click(&mut self) -> bool {
        self.is_click_enabled = !self.is_click_enabled;
        self.is_click_enabled
    }

    pub fn toggle_bank(&mut self) -> PatternBank {
        match self.current_bank {
            PatternBank::A => {
                if !self.bank_b_initialized {
                    self.bank_b = self.bank_a;
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
            let bank = self.get_active_bank_mut();
            bank.tracks[pad_idx].clear();
        }
    }

    pub fn clear_selected_track(&mut self) {
        self.clear_track(self.selected_pad);
    }

    pub fn clear_pattern(&mut self) {
        let bank = self.get_active_bank_mut();
        bank.clear();
    }

    /// Asigna o modifica un paso en la grilla visual mapeándolo a su tick nominal
    pub fn set_step(&mut self, pad_idx: usize, step_idx: usize, velocity: u8) {
        let max_steps = self.resolution.steps_per_bar();
        if pad_idx < NUM_PADS && step_idx < max_steps {
            let target_tick = (step_idx as u32 * self.resolution.ticks_per_step()) % TICKS_PER_BAR;
            let bank = self.get_active_bank_mut();
            bank.tracks[pad_idx].add_or_update(target_tick, target_tick, velocity);
        }
    }

    /// Alterna un paso en la grilla visual (0 <-> 127) mapeado a su tick nominal
    pub fn toggle_step(&mut self, pad_idx: usize, step_idx: usize) -> u8 {
        let max_steps = self.resolution.steps_per_bar();
        if pad_idx < NUM_PADS && step_idx < max_steps {
            self.save_undo_state();
            let target_tick = (step_idx as u32 * self.resolution.ticks_per_step()) % TICKS_PER_BAR;
            let bank = self.get_active_bank_mut();
            if bank.tracks[pad_idx].remove_at_tick(target_tick) {
                0
            } else {
                bank.tracks[pad_idx].add_or_update(target_tick, target_tick, 127);
                127
            }
        } else {
            0
        }
    }

    /// Guarda una instantánea del patrón actual para permitir Undo
    pub fn save_undo_state(&mut self) {
        let current = *self.get_active_bank();
        if self.undo_history.len() >= 16 {
            self.undo_history.remove(0);
        }
        self.undo_history.push(current);
    }

    /// Deshace la última modificación del patrón
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_history.pop() {
            let bank = self.get_active_bank_mut();
            *bank = prev;
            true
        } else {
            false
        }
    }

    /// Graba un golpe tocado en vivo con compensación de latencia, cuantización de entrada y protección anti-flam
    pub fn record_live_hit(&mut self, pad_idx: usize, velocity: u8) -> u32 {
        if pad_idx >= NUM_PADS {
            return self.current_tick;
        }
        // Compensación de latencia: adelantamos el golpe restando el retardo medido
        let raw_tick = self.current_tick as i32 - self.latency_compensation_ticks;
        let orig_tick = ((raw_tick % TICKS_PER_BAR as i32 + TICKS_PER_BAR as i32) % TICKS_PER_BAR as i32) as u32;

        let quantized_tick = if let Some(grid) = self.quantize_mode.grid_ticks() {
            let strength = self.quantize_strength.clamp(0.0, 1.0);
            let nearest = (((orig_tick as f32 / grid as f32).round() as u32) * grid) % TICKS_PER_BAR;
            let diff = (nearest as i32 - orig_tick as i32 + TICKS_PER_BAR as i32 + (TICKS_PER_BAR as i32 / 2)) % (TICKS_PER_BAR as i32) - (TICKS_PER_BAR as i32 / 2);
            let shifted = orig_tick as i32 + (diff as f32 * strength).round() as i32;
            (shifted + TICKS_PER_BAR as i32 * 2) as u32 % TICKS_PER_BAR
        } else {
            orig_tick
        };

        // Anti-Flam: si el golpe se cuantiza hacia adelante en <= 6 ticks (~30 ms a 120 bpm),
        // registrarlo para omitir su redisparo inmediato en esta vuelta del loop
        if quantized_tick > orig_tick && (quantized_tick - orig_tick) <= 6 {
            self.anti_flam_pad = Some(pad_idx);
            self.anti_flam_tick = quantized_tick;
        } else if orig_tick > (TICKS_PER_BAR - 6) && quantized_tick < 6 {
            self.anti_flam_pad = Some(pad_idx);
            self.anti_flam_tick = quantized_tick;
        }

        let bank = self.get_active_bank_mut();
        bank.tracks[pad_idx].add_or_update(quantized_tick, orig_tick, velocity);
        quantized_tick
    }

    /// Re-cuantiza todos los eventos del patrón activo preservando el original_tick
    pub fn requantize_pattern(&mut self) {
        let mode = self.quantize_mode;
        let strength = self.quantize_strength;
        let bank = self.get_active_bank_mut();
        bank.requantize(mode, strength);
    }

    pub fn set_quantize_mode(&mut self, mode: QuantizeMode) {
        self.quantize_mode = mode;
    }

    pub fn set_quantize_strength(&mut self, strength: f32) {
        self.quantize_strength = strength.clamp(0.0, 1.0);
    }

    pub fn set_latency_compensation_ticks(&mut self, ticks: i32) {
        self.latency_compensation_ticks = ticks.clamp(-96, 96);
    }

    /// Devuelve el pulso principal de negra dentro del compás de 4/4 (1..4)
    #[inline]
    pub fn get_current_beat(&self) -> (usize, usize) {
        let beat = ((self.current_tick / TICKS_PER_BEAT) as usize) + 1;
        (beat.clamp(1, 4), 4)
    }

    #[inline]
    fn update_current_step(&mut self) {
        let ticks_step = self.resolution.ticks_per_step();
        let max_steps = self.resolution.steps_per_bar();
        let step = (self.current_tick / ticks_step) as usize;
        self.current_step = step % max_steps;
    }

    #[inline]
    fn trigger_click_if_beat(&mut self) {
        if !self.is_click_enabled {
            return;
        }

        // Clic exacto en cada negra (tick 0, 96, 192, 288)
        if self.current_tick % TICKS_PER_BEAT == 0 {
            let is_downbeat = self.current_tick == 0;
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
                    let calculated_bpm = (60.0 / avg_interval).clamp(40.0, 300.0);
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
            if self.is_paused {
                // Reanuda suavemente desde la posición donde se pausó
                self.is_paused = false;
                self.need_start_trigger = false;
            } else {
                // Arranca desde Stop (tick 0) con disparo inmediato
                self.current_tick = 0;
                self.sample_accum = 0.0;
                self.update_current_step();
                self.need_start_trigger = true;
            }
        }
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
        self.is_paused = true;
        self.need_start_trigger = false;
        self.click_env = 0.0;
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.is_paused = false;
        self.current_tick = 0;
        self.current_step = 0;
        self.sample_accum = 0.0;
        self.need_start_trigger = false;
        self.click_env = 0.0;
        self.anti_flam_pad = None;
    }

    pub fn toggle_play(&mut self) -> bool {
        if self.is_playing {
            self.pause();
            false
        } else {
            self.play();
            true
        }
    }

    pub fn set_recording(&mut self, rec: bool) {
        self.is_recording = rec;
    }

    pub fn toggle_recording(&mut self) -> bool {
        self.is_recording = !self.is_recording;
        self.is_recording
    }

    /// Calcula el paso destino cuantizado al 50% de la celda de paso actual
    #[allow(dead_code)]
    pub fn get_quantized_step(&self) -> usize {
        let ticks_step = self.resolution.ticks_per_step();
        let max_steps = self.resolution.steps_per_bar();
        let step = ((self.current_tick as f32 + (ticks_step as f32 / 2.0)) / ticks_step as f32).floor() as usize;
        step % max_steps
    }

    pub fn get_state_info(&self) -> SequencerStateInfo {
        SequencerStateInfo {
            is_playing: self.is_playing,
            is_paused: self.is_paused,
            is_recording: self.is_recording,
            bpm: self.bpm,
            current_step: self.current_step,
            resolution: self.resolution,
            current_bank: self.current_bank,
            is_click_enabled: self.is_click_enabled,
            selected_pad: self.selected_pad,
            quantize_mode: self.quantize_mode,
            quantize_strength: self.quantize_strength,
            latency_compensation_ticks: self.latency_compensation_ticks,
            pattern: self.get_active_pattern(),
        }
    }

    /// Avanza un frame de audio contando ticks sample-accurate.
    /// Retorna Some((nuevo_paso_visual, triggers, count)) si ocurrió un disparo o cambio de paso.
    #[inline]
    pub fn advance_sample(&mut self) -> Option<(usize, [(usize, f32); NUM_PADS], usize)> {
        if !self.is_playing {
            return None;
        }

        // 1. Disparo instantáneo en el tick 0 exacto al presionar Play (latencia 0 ms)
        if self.need_start_trigger {
            self.need_start_trigger = false;
            self.current_tick = 0;
            self.sample_accum = 0.0;
            self.update_current_step();
            self.trigger_click_if_beat();

            let mut count = 0;
            let mut triggers = [(0, 0.0f32); NUM_PADS];
            let bank = self.get_active_bank();
            for pad_idx in 0..NUM_PADS {
                let track = &bank.tracks[pad_idx];
                for i in 0..track.count {
                    if track.events[i].tick == 0 {
                        triggers[count] = (pad_idx, track.events[i].velocity as f32 / 127.0);
                        count += 1;
                        break;
                    }
                }
            }

            return Some((self.current_step, triggers, count));
        }

        // 2. Acumulación fraccionaria de muestras por tick
        self.sample_accum += 1.0;
        if self.sample_accum >= self.samples_per_tick {
            self.sample_accum -= self.samples_per_tick;
            
            let prev_step = self.current_step;
            self.current_tick = (self.current_tick + 1) % TICKS_PER_BAR;
            self.update_current_step();
            self.trigger_click_if_beat();

            // Buscar si hay eventos programados en este tick
            let mut suppress_pad = None;
            if self.anti_flam_tick == self.current_tick {
                suppress_pad = self.anti_flam_pad.take();
            }

            let mut count = 0;
            let mut triggers = [(0, 0.0f32); NUM_PADS];
            let bank = self.get_active_bank();
            for pad_idx in 0..NUM_PADS {
                // Anti-Flam: si este pad fue golpeado en vivo y cuantizado hacia adelante a este tick, suprimir
                if suppress_pad == Some(pad_idx) {
                    continue;
                }

                let track = &bank.tracks[pad_idx];
                for i in 0..track.count {
                    if track.events[i].tick == self.current_tick {
                        triggers[count] = (pad_idx, track.events[i].velocity as f32 / 127.0);
                        count += 1;
                        break;
                    }
                }
            }

            // Notificar a la UI si hay disparos o si cambió el paso visual
            if count > 0 || self.current_step != prev_step {
                Some((self.current_step, triggers, count))
            } else {
                None
            }
        } else {
            None
        }
    }
}
