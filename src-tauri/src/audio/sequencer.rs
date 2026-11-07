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

    #[inline]
    pub fn has_events(&self) -> bool {
        self.count > 0
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

    pub fn has_projected_step(&self, step_idx: usize, resolution: StepResolution) -> bool {
        for i in 0..self.count {
            let ev = self.events[i];
            if ev.velocity > 0 && resolution.project_tick_to_step(ev.tick) == step_idx {
                return true;
            }
        }
        false
    }

    pub fn remove_projected_step(&mut self, step_idx: usize, resolution: StepResolution) -> bool {
        let mut removed = false;
        let mut i = 0;
        while i < self.count {
            if resolution.project_tick_to_step(self.events[i].tick) == step_idx {
                self.remove_at(i);
                removed = true;
            } else {
                i += 1;
            }
        }
        removed
    }

    pub fn requantize(&mut self, mode: QuantizeMode, strength: f32) -> bool {
        let mut changed = false;
        if let Some(grid) = mode.grid_ticks() {
            let strength = strength.clamp(0.0, 1.0);
            for i in 0..self.count {
                let orig = self.events[i].original_tick % TICKS_PER_BAR;
                let nearest = (((orig as f32 / grid as f32).round() as u32) * grid) % TICKS_PER_BAR;
                let diff = (nearest as i32 - orig as i32 + TICKS_PER_BAR as i32 + (TICKS_PER_BAR as i32 / 2)) % (TICKS_PER_BAR as i32) - (TICKS_PER_BAR as i32 / 2);
                let shifted = orig as i32 + (diff as f32 * strength).round() as i32;
                let quantized = (shifted + TICKS_PER_BAR as i32 * 2) as u32 % TICKS_PER_BAR;
                if self.events[i].tick != quantized {
                    self.events[i].tick = quantized;
                    changed = true;
                }
            }
        } else {
            for i in 0..self.count {
                if self.events[i].tick != self.events[i].original_tick {
                    self.events[i].tick = self.events[i].original_tick;
                    changed = true;
                }
            }
        }
        changed
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

    #[inline]
    pub fn has_events(&self) -> bool {
        self.tracks.iter().any(|t| t.has_events())
    }

    pub fn clear(&mut self) {
        for t in self.tracks.iter_mut() {
            t.clear();
        }
    }

    pub fn requantize(&mut self, mode: QuantizeMode, strength: f32) -> bool {
        let mut changed = false;
        for t in self.tracks.iter_mut() {
            if t.requantize(mode, strength) {
                changed = true;
            }
        }
        changed
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

    #[inline]
    pub fn project_tick_to_step(&self, tick: u32) -> usize {
        let ticks_per_step = self.ticks_per_step();
        let total_steps = self.steps_per_bar();
        (((tick % TICKS_PER_BAR) + (ticks_per_step / 2)) / ticks_per_step) as usize % total_steps
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
    pub anti_flam_ticks: [Option<u32>; NUM_PADS],
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
    undo_history: Vec<(PatternBank, BankPattern)>,
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
            anti_flam_ticks: [None; NUM_PADS],
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
        let resolution = self.resolution;
        let max_steps = resolution.steps_per_bar();

        for pad_idx in 0..NUM_PADS {
            let track = &bank.tracks[pad_idx];
            for i in 0..track.count {
                let ev = track.events[i];
                // Asignar a la celda de paso más cercana con proyección circular (ventana +- 50% del paso)
                let step_idx = resolution.project_tick_to_step(ev.tick);
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
        self.anti_flam_ticks = [None; NUM_PADS];
        self.current_bank
    }

    pub fn set_selected_pad(&mut self, pad_idx: usize) {
        if pad_idx < NUM_PADS {
            self.selected_pad = pad_idx;
        }
    }

    pub fn clear_track(&mut self, pad_idx: usize) {
        if pad_idx < NUM_PADS {
            self.anti_flam_ticks[pad_idx] = None;
            if self.get_active_bank().tracks[pad_idx].has_events() {
                self.save_undo_state();
                let bank = self.get_active_bank_mut();
                bank.tracks[pad_idx].clear();
            }
        }
    }

    pub fn clear_selected_track(&mut self) {
        self.clear_track(self.selected_pad);
    }

    pub fn clear_pattern(&mut self) {
        self.anti_flam_ticks = [None; NUM_PADS];
        if self.get_active_bank().has_events() {
            self.save_undo_state();
            let bank = self.get_active_bank_mut();
            bank.clear();
        }
    }

    /// Asigna o modifica un paso en la grilla visual mapeándolo a su celda proyectada o tick nominal
    pub fn set_step(&mut self, pad_idx: usize, step_idx: usize, velocity: u8, save_undo: bool) {
        let resolution = self.resolution;
        let max_steps = resolution.steps_per_bar();
        if pad_idx < NUM_PADS && step_idx < max_steps {
            let target_tick = (step_idx as u32 * resolution.ticks_per_step()) % TICKS_PER_BAR;
            let already_active = self.get_active_bank().tracks[pad_idx].has_projected_step(step_idx, resolution);

            let target_active = velocity > 0;
            if already_active == target_active {
                return;
            }

            if save_undo {
                self.save_undo_state();
            }

            let bank = self.get_active_bank_mut();
            if target_active {
                bank.tracks[pad_idx].add_or_update(target_tick, target_tick, velocity);
            } else {
                bank.tracks[pad_idx].remove_projected_step(step_idx, resolution);
            }
        }
    }

    /// Alterna un paso en la grilla visual (0 <-> 127) limpiando todos los eventos proyectados en la celda al apagar
    pub fn toggle_step(&mut self, pad_idx: usize, step_idx: usize) -> u8 {
        let resolution = self.resolution;
        let max_steps = resolution.steps_per_bar();
        if pad_idx < NUM_PADS && step_idx < max_steps {
            self.save_undo_state();
            let target_tick = (step_idx as u32 * resolution.ticks_per_step()) % TICKS_PER_BAR;
            let bank = self.get_active_bank_mut();
            if bank.tracks[pad_idx].remove_projected_step(step_idx, resolution) {
                0
            } else {
                bank.tracks[pad_idx].add_or_update(target_tick, target_tick, 127);
                127
            }
        } else {
            0
        }
    }

    /// Guarda una instantánea del patrón actual junto con su banco de origen para permitir Undo
    pub fn save_undo_state(&mut self) {
        let bank_id = self.current_bank;
        let current = *self.get_active_bank();
        if self.undo_history.len() >= 16 {
            self.undo_history.remove(0);
        }
        self.undo_history.push((bank_id, current));
    }

    /// Deshace la última modificación restaurando el patrón sobre su banco de origen y sincronizando el banco activo
    pub fn undo(&mut self) -> bool {
        if let Some((bank_id, prev)) = self.undo_history.pop() {
            match bank_id {
                PatternBank::A => {
                    self.bank_a = prev;
                }
                PatternBank::B => {
                    self.bank_b = prev;
                    self.bank_b_initialized = true;
                }
            }
            self.current_bank = bank_id;
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
        // registrarlo en el slot independiente de este pad para omitir su redisparo inmediato en esta vuelta del loop
        let forward_dist = (quantized_tick + TICKS_PER_BAR - orig_tick) % TICKS_PER_BAR;
        if forward_dist > 0 && forward_dist <= 6 {
            self.anti_flam_ticks[pad_idx] = Some(quantized_tick);
        } else {
            self.anti_flam_ticks[pad_idx] = None;
        }

        let bank = self.get_active_bank_mut();
        bank.tracks[pad_idx].add_or_update(quantized_tick, orig_tick, velocity);
        quantized_tick
    }

    /// Re-cuantiza todos los eventos del patrón activo preservando el original_tick
    pub fn requantize_pattern(&mut self) {
        let mode = self.quantize_mode;
        let strength = self.quantize_strength;
        let mut preview = *self.get_active_bank();
        if preview.requantize(mode, strength) {
            self.save_undo_state();
            *self.get_active_bank_mut() = preview;
        }
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
                self.anti_flam_ticks = [None; NUM_PADS];
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
        self.anti_flam_ticks = [None; NUM_PADS];
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.is_paused = false;
        self.current_tick = 0;
        self.current_step = 0;
        self.sample_accum = 0.0;
        self.need_start_trigger = false;
        self.click_env = 0.0;
        self.anti_flam_ticks = [None; NUM_PADS];
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
        self.resolution.project_tick_to_step(self.current_tick)
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
            let current_tick = self.current_tick;
            let mut suppress_pads = [false; NUM_PADS];
            for pad_idx in 0..NUM_PADS {
                if self.anti_flam_ticks[pad_idx] == Some(current_tick) {
                    self.anti_flam_ticks[pad_idx] = None;
                    suppress_pads[pad_idx] = true;
                }
            }

            let mut count = 0;
            let mut triggers = [(0, 0.0f32); NUM_PADS];
            let bank = self.get_active_bank();
            for pad_idx in 0..NUM_PADS {
                // Anti-Flam: si este pad fue golpeado en vivo y cuantizado hacia adelante a este tick, suprimir
                if suppress_pads[pad_idx] {
                    continue;
                }

                let track = &bank.tracks[pad_idx];
                for i in 0..track.count {
                    if track.events[i].tick == current_tick {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_empty_history() {
        let mut seq = StepSequencer::new(48000.0);
        assert!(!seq.undo());
        assert_eq!(seq.current_bank, PatternBank::A);
    }

    #[test]
    fn test_undo_preserves_bank_identity_across_bank_switch() {
        let mut seq = StepSequencer::new(48000.0);

        // 1. Estado base en Banco A: paso 0 activo en pad 0
        seq.toggle_step(0, 0);
        assert_eq!(seq.get_active_pattern()[0][0], 127);

        // 2. Segunda edición en Banco A: paso 4 activo en pad 0 (se guarda snapshot con solo paso 0)
        seq.toggle_step(0, 4);
        assert_eq!(seq.get_active_pattern()[0][4], 127);

        // 3. Cambiar a Banco B (se inicializa como copia de A y luego editamos paso 8 en B, o lo dejamos distinto)
        assert_eq!(seq.toggle_bank(), PatternBank::B);
        seq.bank_b.tracks[1].add_or_update(48, 48, 127); // modificación directa en B sin tocar A
        let expected_b_track1_count = seq.bank_b.tracks[1].count;

        // 4. Ejecutar Undo estando posicionado en el Banco B:
        // Debe restaurar la última edición (que ocurrió en A), sincronizar current_bank a A,
        // y dejar el Banco B completamente intacto.
        assert!(seq.undo());
        assert_eq!(seq.current_bank, PatternBank::A);
        let pat_a = seq.get_active_pattern();
        assert_eq!(pat_a[0][0], 127);
        assert_eq!(pat_a[0][4], 0);

        // Verificar que Banco B permanece intacto
        assert_eq!(seq.bank_b.tracks[1].count, expected_b_track1_count);
        assert_eq!(seq.bank_b.tracks[0].count, 2);
    }

    #[test]
    fn test_undo_after_clear_track_and_noop_guard() {
        let mut seq = StepSequencer::new(48000.0);

        // Limpiar pista vacía no debe generar entrada de Undo
        seq.clear_track(0);
        assert!(!seq.undo());

        // Añadir pasos en pista 0 y pista 1
        seq.toggle_step(0, 0);
        seq.toggle_step(1, 2);
        let history_before = seq.undo_history.len();

        // Limpiar pista 2 (vacía) -> no debe agregar snapshot al historial
        seq.clear_track(2);
        assert_eq!(seq.undo_history.len(), history_before);

        // Limpiar pista 0 (con eventos) -> sí guarda snapshot y borra pista 0
        seq.clear_track(0);
        assert_eq!(seq.undo_history.len(), history_before + 1);
        assert_eq!(seq.get_active_pattern()[0][0], 0);
        assert_eq!(seq.get_active_pattern()[1][2], 127);

        // Ejecutar Undo -> restaura pista 0
        assert!(seq.undo());
        assert_eq!(seq.get_active_pattern()[0][0], 127);
        assert_eq!(seq.get_active_pattern()[1][2], 127);
    }

    #[test]
    fn test_undo_after_clear_pattern_and_noop_guard() {
        let mut seq = StepSequencer::new(48000.0);

        // clear_pattern sobre patrón vacío no debe generar entrada de Undo
        seq.clear_pattern();
        assert_eq!(seq.undo_history.len(), 0);
        assert!(!seq.undo());

        // Poblar patrón en varios pads
        seq.toggle_step(0, 1);
        seq.toggle_step(3, 5);
        let history_before = seq.undo_history.len();

        // Ejecutar clear_pattern
        seq.clear_pattern();
        assert_eq!(seq.undo_history.len(), history_before + 1);
        assert!(!seq.get_active_bank().has_events());

        // Repetir clear_pattern estando ya vacío no debe apilar otra entrada
        seq.clear_pattern();
        assert_eq!(seq.undo_history.len(), history_before + 1);

        // Ejecutar Undo -> recupera todo el patrón previo
        assert!(seq.undo());
        let pat = seq.get_active_pattern();
        assert_eq!(pat[0][1], 127);
        assert_eq!(pat[3][5], 127);
    }

    #[test]
    fn test_undo_after_requantize_pattern_and_noop_guard() {
        let mut seq = StepSequencer::new(48000.0);

        // Requantize en patrón vacío no debe generar snapshot
        seq.requantize_pattern();
        assert_eq!(seq.undo_history.len(), 0);

        // Grabar un evento off-grid (tick 20, original_tick 20) con QuantizeMode::Off
        seq.set_quantize_mode(QuantizeMode::Off);
        seq.current_tick = 20;
        seq.save_undo_state();
        seq.record_live_hit(0, 120);
        assert_eq!(seq.get_active_bank().tracks[0].events[0].tick, 20);
        let history_before = seq.undo_history.len();

        // Requantize con QuantizeMode::Off no cambia ningún tick -> no debe generar snapshot
        seq.requantize_pattern();
        assert_eq!(seq.undo_history.len(), history_before);

        // Cambiar a QuantizeMode::Sixteenth (grid = 24) con fuerza 1.0 y recuantizar
        seq.set_quantize_mode(QuantizeMode::Sixteenth);
        seq.set_quantize_strength(1.0);
        seq.requantize_pattern();
        assert_eq!(seq.undo_history.len(), history_before + 1);
        assert_eq!(seq.get_active_bank().tracks[0].events[0].tick, 24);

        // Volver a llamar requantize_pattern con los mismos parámetros (ya está en 24) -> no-op
        seq.requantize_pattern();
        assert_eq!(seq.undo_history.len(), history_before + 1);

        // Undo debe restaurar el evento a su tick pre-requantize (20)
        assert!(seq.undo());
        assert_eq!(seq.get_active_bank().tracks[0].events[0].tick, 20);
    }

    fn advance_one_tick(seq: &mut StepSequencer) -> Option<(usize, [(usize, f32); NUM_PADS], usize)> {
        seq.sample_accum = seq.samples_per_tick;
        seq.advance_sample()
    }

    #[test]
    fn test_anti_flam_simultaneous_kick_and_snare() {
        let mut seq = StepSequencer::new(48000.0);
        seq.is_playing = true;
        seq.is_recording = true;
        seq.set_quantize_mode(QuantizeMode::Sixteenth);
        seq.set_quantize_strength(1.0);

        // En tick 22, Kick (pad 0) y Snare (pad 1) se golpean simultáneamente -> ambos cuantizan a tick 24
        seq.current_tick = 22;
        assert_eq!(seq.record_live_hit(0, 127), 24);
        assert_eq!(seq.record_live_hit(1, 127), 24);
        assert_eq!(seq.anti_flam_ticks[0], Some(24));
        assert_eq!(seq.anti_flam_ticks[1], Some(24));

        // Avanzar a tick 23 y luego a tick 24: ninguno de los dos debe redispararse en esta vuelta
        let _ = advance_one_tick(&mut seq); // tick 23
        let res_24 = advance_one_tick(&mut seq); // tick 24 (cambio de paso visual, pero 0 triggers)
        if let Some((_step, _triggers, count)) = res_24 {
            assert_eq!(count, 0, "Ni Kick ni Snare deben redispararse en tick 24");
        }
        assert_eq!(seq.anti_flam_ticks[0], None);
        assert_eq!(seq.anti_flam_ticks[1], None);

        // En la siguiente vuelta del compás, al llegar a tick 24 ambos sí deben sonar
        seq.current_tick = 23;
        let (_, triggers, count) = advance_one_tick(&mut seq).expect("Debe disparar en la siguiente vuelta");
        assert_eq!(count, 2);
        assert_eq!(triggers[0].0, 0);
        assert_eq!(triggers[1].0, 1);
    }

    #[test]
    fn test_anti_flam_simultaneous_three_or_more_pads() {
        let mut seq = StepSequencer::new(48000.0);
        seq.is_playing = true;
        seq.is_recording = true;
        seq.set_quantize_mode(QuantizeMode::Sixteenth);
        seq.set_quantize_strength(1.0);

        // Golpear 4 pads simultáneamente en tick 45 -> cuantizan a tick 48
        seq.current_tick = 45;
        for &pad in &[0usize, 1, 2, 7] {
            assert_eq!(seq.record_live_hit(pad, 127), 48);
            assert_eq!(seq.anti_flam_ticks[pad], Some(48));
        }

        seq.current_tick = 47;
        let res_48 = advance_one_tick(&mut seq);
        if let Some((_step, _triggers, count)) = res_48 {
            assert_eq!(count, 0, "Los 4 pads deben ser suprimidos por anti-flam en tick 48");
        }
        for &pad in &[0usize, 1, 2, 7] {
            assert_eq!(seq.anti_flam_ticks[pad], None);
        }
    }

    #[test]
    fn test_anti_flam_two_rapid_consecutive_hits_same_pad_and_different_pads_in_window() {
        let mut seq = StepSequencer::new(48000.0);
        seq.is_playing = true;
        seq.is_recording = true;
        seq.set_quantize_mode(QuantizeMode::Sixteenth);
        seq.set_quantize_strength(1.0);

        // 1. Dos golpes rápidos consecutivos del mismo pad (pad 0 en tick 20 y luego en tick 22 -> ambos van a 24)
        seq.current_tick = 20;
        assert_eq!(seq.record_live_hit(0, 100), 24);
        assert_eq!(seq.anti_flam_ticks[0], Some(24));

        seq.current_tick = 22;
        assert_eq!(seq.record_live_hit(0, 127), 24);
        assert_eq!(seq.anti_flam_ticks[0], Some(24));
        // Además, otro pad distinto (pad 3) se golpea en tick 23 dentro de la misma ventana Anti-Flam (-> 24)
        seq.current_tick = 23;
        assert_eq!(seq.record_live_hit(3, 110), 24);
        assert_eq!(seq.anti_flam_ticks[3], Some(24));

        // Al cruzar tick 24, tanto pad 0 como pad 3 son suprimidos
        let res_24 = advance_one_tick(&mut seq);
        if let Some((_step, _triggers, count)) = res_24 {
            assert_eq!(count, 0);
        }
        assert_eq!(seq.anti_flam_ticks[0], None);
        assert_eq!(seq.anti_flam_ticks[3], None);
    }

    #[test]
    fn test_anti_flam_bar_boundary_wrap_and_transport_reset() {
        let mut seq = StepSequencer::new(48000.0);
        seq.is_playing = true;
        seq.is_recording = true;
        seq.set_quantize_mode(QuantizeMode::Sixteenth);
        seq.set_quantize_strength(1.0);

        // Golpear pads 0 y 2 en tick 380 (a 4 ticks del final del compás 384) -> cuantizan a tick 0
        seq.current_tick = 380;
        assert_eq!(seq.record_live_hit(0, 127), 0);
        assert_eq!(seq.record_live_hit(2, 127), 0);
        assert_eq!(seq.anti_flam_ticks[0], Some(0));
        assert_eq!(seq.anti_flam_ticks[2], Some(0));

        // Avanzar desde 383 al tick 0 (cruce de límite de compás): ambos deben suprimirse
        seq.current_tick = 383;
        let res_0 = advance_one_tick(&mut seq);
        if let Some((_step, _triggers, count)) = res_0 {
            assert_eq!(count, 0, "Cruzar el límite de compás hacia tick 0 debe respetar anti-flam");
        }
        assert_eq!(seq.anti_flam_ticks[0], None);
        assert_eq!(seq.anti_flam_ticks[2], None);

        // Verificar limpieza en detención y reinicio del transporte (stop / play)
        seq.current_tick = 20;
        seq.record_live_hit(1, 127); // marca Some(24)
        assert_eq!(seq.anti_flam_ticks[1], Some(24));
        seq.stop();
        assert_eq!(seq.anti_flam_ticks, [None; NUM_PADS]);

        // Reiniciar transporte y verificar que tick 0 dispara normalmente
        seq.play();
        let (_, triggers, count) = seq.advance_sample().expect("Start trigger en tick 0");
        assert_eq!(count, 2);
        assert_eq!(triggers[0].0, 0);
        assert_eq!(triggers[1].0, 2);
    }

    #[test]
    fn test_anti_flam_quantize_off_no_regression() {
        let mut seq = StepSequencer::new(48000.0);
        seq.is_playing = true;
        seq.is_recording = true;
        seq.set_quantize_mode(QuantizeMode::Off);

        // Con QuantizeMode::Off, quantized_tick == orig_tick (distancia hacia adelante = 0)
        seq.current_tick = 22;
        assert_eq!(seq.record_live_hit(0, 127), 22);
        assert_eq!(seq.record_live_hit(1, 127), 22);
        assert_eq!(seq.anti_flam_ticks[0], None);
        assert_eq!(seq.anti_flam_ticks[1], None);
    }

    #[test]
    fn test_circular_projection_tick_383_and_tick_23() {
        let mut seq = StepSequencer::new(48000.0);
        seq.set_resolution(StepResolution::Sixteenth); // 16 pasos, 24 ticks/paso
        seq.set_quantize_mode(QuantizeMode::Off);

        // Tick 383 debe proyectarse circularmente sobre el paso 0 en grilla de 16 pasos
        assert_eq!(StepResolution::Sixteenth.project_tick_to_step(383), 0);
        // Tick 23 debe proyectarse sobre el paso 1 (nominal tick 24)
        assert_eq!(StepResolution::Sixteenth.project_tick_to_step(23), 1);
        // También verificar que en 1/8 (8 pasos, 48 ticks/paso) y 1/32 (32 pasos, 12 ticks/paso) tick 383 proyecta a 0
        assert_eq!(StepResolution::Eighth.project_tick_to_step(383), 0);
        assert_eq!(StepResolution::ThirtySecond.project_tick_to_step(383), 0);

        // Grabar en tick 383 (pad 0) y tick 23 (pad 1) y verificar visibilidad en get_active_pattern
        seq.current_tick = 383;
        seq.record_live_hit(0, 115);
        seq.current_tick = 23;
        seq.record_live_hit(1, 105);

        let pat = seq.get_active_pattern();
        assert_eq!(pat[0][0], 115, "Tick 383 debe iluminar el paso 0");
        assert_eq!(pat[1][1], 105, "Tick 23 debe iluminar el paso 1");
    }

    #[test]
    fn test_delete_off_grid_and_multiple_events_per_cell_preserving_others_and_undo() {
        let mut seq = StepSequencer::new(48000.0);
        seq.set_resolution(StepResolution::Sixteenth); // 24 ticks/paso
        seq.set_quantize_mode(QuantizeMode::Off);

        // 1. Grabar evento off-grid en tick 23 (proyecta a paso 1) en pad 0
        seq.current_tick = 23;
        seq.save_undo_state();
        seq.record_live_hit(0, 120);
        assert_eq!(seq.get_active_pattern()[0][1], 120);

        // 2. Grabar evento en otra celda (tick 50 -> proyecta a paso 2, nominal 48) en pad 0
        // y otro evento en otra pista (pad 2, tick 23 -> paso 1)
        seq.current_tick = 50;
        seq.save_undo_state();
        seq.record_live_hit(0, 110);
        seq.current_tick = 23;
        seq.save_undo_state();
        seq.record_live_hit(2, 100);

        // 3. Borrar evento off-grid del paso 1 en pad 0 usando toggle_step
        assert_eq!(seq.toggle_step(0, 1), 0, "toggle_step sobre celda iluminada por evento off-grid debe apagarla");
        let pat_after_toggle = seq.get_active_pattern();
        assert_eq!(pat_after_toggle[0][1], 0, "Celda 1 de pad 0 debe quedar apagada");
        assert_eq!(pat_after_toggle[0][2], 110, "Celda 2 de pad 0 debe conservarse intacta");
        assert_eq!(pat_after_toggle[2][1], 100, "Pista 2 debe conservarse intacta");
        // Verificar que el microtiming original (tick 50) de la celda 2 no fue alterado
        assert_eq!(seq.get_active_bank().tracks[0].count, 1);
        assert_eq!(seq.get_active_bank().tracks[0].events[0].tick, 50);

        // 4. Undo después del borrado off-grid debe restaurar exactamente el evento en tick 23
        assert!(seq.undo());
        assert_eq!(seq.get_active_pattern()[0][1], 120);
        assert_eq!(seq.get_active_bank().tracks[0].count, 2);
        assert_eq!(seq.get_active_bank().tracks[0].events[0].tick, 23);

        // 5. Múltiples eventos proyectados sobre una misma celda (ej. ticks 376, 383 y 5 -> todos proyectan al paso 0)
        seq.clear_pattern();
        seq.get_active_bank_mut().tracks[0].add_or_update(376, 376, 90);
        seq.get_active_bank_mut().tracks[0].add_or_update(383, 383, 100);
        seq.get_active_bank_mut().tracks[0].add_or_update(5, 5, 127);
        seq.get_active_bank_mut().tracks[0].add_or_update(25, 25, 115); // paso 1
        assert_eq!(seq.get_active_bank().tracks[0].count, 4);
        assert_eq!(seq.get_active_pattern()[0][0], 127);

        // Apagar el paso 0 mediante set_step(0, 0, 0, true) (erase drag)
        seq.set_step(0, 0, 0, true);
        assert_eq!(seq.get_active_pattern()[0][0], 0, "Todos los eventos proyectados al paso 0 deben eliminarse");
        assert_eq!(seq.get_active_bank().tracks[0].count, 1, "Solo debe quedar el evento del paso 1");
        assert_eq!(seq.get_active_bank().tracks[0].events[0].tick, 25);

        // Undo restaura los 3 eventos del paso 0 (incluyendo los del borde circular 376 y 383)
        assert!(seq.undo());
        assert_eq!(seq.get_active_bank().tracks[0].count, 4);
        assert_eq!(seq.get_active_pattern()[0][0], 127);

        // 6. Verificar que encender un paso vacío conserva su comportamiento nominal
        seq.toggle_step(0, 4); // paso 4 -> tick nominal 96
        assert_eq!(seq.get_active_pattern()[0][4], 127);
        let track0 = &seq.get_active_bank().tracks[0];
        assert!(track0.events[..track0.count].iter().any(|e| e.tick == 96 && e.velocity == 127));
    }
}
