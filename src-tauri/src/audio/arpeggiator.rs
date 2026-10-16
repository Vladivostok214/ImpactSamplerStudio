use serde::{Deserialize, Serialize};

/// Modos de patrón melódico esenciales del arpegiador
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArpMode {
    Up,     // De la nota más grave a la más aguda
    Down,   // De la nota más aguda a la más grave
    UpDown, // Sube y baja a través del acorde
}

/// Divisiones de tempo rectas sincronizadas al reloj de 96 ticks/negra
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArpRate {
    Quarter,     // 1/4 (96 ticks)
    Eighth,      // 1/8 (48 ticks)
    Sixteenth,   // 1/16 (24 ticks - default)
    ThirtySecond // 1/32 (12 ticks)
}

impl ArpRate {
    #[inline]
    pub fn ticks_per_step(&self) -> u32 {
        match self {
            ArpRate::Quarter => 96,
            ArpRate::Eighth => 48,
            ArpRate::Sixteenth => 24,
            ArpRate::ThirtySecond => 12,
        }
    }
}

/// Estado exportable hacia el frontend
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArpStateInfo {
    pub enabled: bool,
    pub hold: bool,
    pub mode: ArpMode,
    pub rate: ArpRate,
    pub octaves: u8,
}

/// Evento discreto generado por el arpegiador para el sintetizador melódico
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpOutputEvent {
    NoteTrigger { note: u8, velocity: u8 },
    NoteRelease { note: u8 },
}

/// Motor de Arpegiador Melódico en memoria plana (Zero-Alloc & Lock-Free)
#[derive(Clone)]
pub struct Arpeggiator {
    pub enabled: bool,
    pub hold: bool,
    pub mode: ArpMode,
    pub rate: ArpRate,
    pub octaves: u8, // 1, 2 o 3 octavas

    // Buffer de teclas físicas presionadas actualmente (hasta 16)
    pub held_notes: [Option<u8>; 16],
    pub held_velocities: [u8; 16],
    pub held_count: usize,

    // Buffer de notas retenidas para el modo HOLD
    pub hold_notes: [Option<u8>; 16],
    pub hold_velocities: [u8; 16],
    pub hold_count: usize,

    // Secuencia generada a ejecutar (16 notas * 3 octavas = 48 máximo)
    pub sequence: [u8; 48],
    pub sequence_velocities: [u8; 48],
    pub sequence_len: usize,
    pub seq_index: usize,

    // Parámetros de reloj de audio
    pub sample_rate_f: f32,
    pub sample_accumulator: f64,
    pub tick_accumulator: u32,

    // Articulación y duración de la nota (Gate 80%)
    pub active_playing_note: Option<u8>,
    pub note_duration_samples: usize,
    pub note_sample_counter: usize,
}

impl Arpeggiator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            enabled: false,
            hold: false,
            mode: ArpMode::Up,
            rate: ArpRate::Sixteenth,
            octaves: 1,

            held_notes: [None; 16],
            held_velocities: [0; 16],
            held_count: 0,

            hold_notes: [None; 16],
            hold_velocities: [0; 16],
            hold_count: 0,

            sequence: [0; 48],
            sequence_velocities: [0; 48],
            sequence_len: 0,
            seq_index: 0,

            sample_rate_f: sample_rate,
            sample_accumulator: 0.0,
            tick_accumulator: 0,

            active_playing_note: None,
            note_duration_samples: 0,
            note_sample_counter: 0,
        }
    }

    pub fn update_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate_f = sample_rate;
    }

    /// Retorna el estado actual para la interfaz
    pub fn get_state_info(&self) -> ArpStateInfo {
        ArpStateInfo {
            enabled: self.enabled,
            hold: self.hold,
            mode: self.mode,
            rate: self.rate,
            octaves: self.octaves,
        }
    }

    /// Manejo de NoteOn: agrega la nota y regenera la secuencia
    pub fn handle_note_on(&mut self, note: u8, velocity: u8) {
        // Si HOLD está activo pero todas las teclas físicas anteriores ya se habían soltado,
        // esta nueva pulsación inicia un nuevo acorde limpio (re-trigger inteligente)
        if self.hold && self.held_count == 0 && self.hold_count > 0 {
            self.hold_notes = [None; 16];
            self.hold_velocities = [0; 16];
            self.hold_count = 0;
            self.seq_index = 0;
            self.tick_accumulator = self.rate.ticks_per_step(); // Disparo inmediato en el primer tick
        }

        // Buscar si la nota ya estaba en held_notes
        let mut exists = false;
        for i in 0..self.held_count {
            if self.held_notes[i] == Some(note) {
                self.held_velocities[i] = velocity;
                exists = true;
                break;
            }
        }

        if !exists && self.held_count < 16 {
            self.held_notes[self.held_count] = Some(note);
            self.held_velocities[self.held_count] = velocity;
            self.held_count += 1;
        }

        // Si hold está activo, sincronizar con hold_notes
        if self.hold {
            let mut hold_exists = false;
            for i in 0..self.hold_count {
                if self.hold_notes[i] == Some(note) {
                    self.hold_velocities[i] = velocity;
                    hold_exists = true;
                    break;
                }
            }
            if !hold_exists && self.hold_count < 16 {
                self.hold_notes[self.hold_count] = Some(note);
                self.hold_velocities[self.hold_count] = velocity;
                self.hold_count += 1;
            }
        }

        self.rebuild_sequence();
    }

    /// Manejo de NoteOff: retira la nota de held_notes
    pub fn handle_note_off(&mut self, note: u8) {
        let mut found_idx = None;
        for i in 0..self.held_count {
            if self.held_notes[i] == Some(note) {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            for i in idx..self.held_count - 1 {
                self.held_notes[i] = self.held_notes[i + 1];
                self.held_velocities[i] = self.held_velocities[i + 1];
            }
            self.held_notes[self.held_count - 1] = None;
            self.held_velocities[self.held_count - 1] = 0;
            self.held_count -= 1;
        }

        // Si HOLD está desactivado y se soltaron todas las teclas, vaciar secuencia
        if !self.hold {
            if self.held_count == 0 {
                self.sequence_len = 0;
                self.seq_index = 0;
            } else {
                self.rebuild_sequence();
            }
        }
    }

    /// Reconstruye la secuencia de notas ordenada según modo y rango de octavas
    pub fn rebuild_sequence(&mut self) {
        let (source_notes, source_vels, source_count) = if self.held_count > 0 {
            (&self.held_notes, &self.held_velocities, self.held_count)
        } else if self.hold && self.hold_count > 0 {
            (&self.hold_notes, &self.hold_velocities, self.hold_count)
        } else {
            self.sequence_len = 0;
            self.seq_index = 0;
            return;
        };

        if source_count == 0 {
            self.sequence_len = 0;
            self.seq_index = 0;
            return;
        }

        let mut base_notes = [0u8; 16];
        let mut base_vels = [0u8; 16];
        let mut n = 0;
        for i in 0..source_count {
            if let Some(note) = source_notes[i] {
                base_notes[n] = note;
                base_vels[n] = source_vels[i];
                n += 1;
            }
        }

        if n == 0 {
            self.sequence_len = 0;
            self.seq_index = 0;
            return;
        }

        // Insertion Sort in-place
        for i in 1..n {
            let key_note = base_notes[i];
            let key_vel = base_vels[i];
            let mut j = i;
            while j > 0 && base_notes[j - 1] > key_note {
                base_notes[j] = base_notes[j - 1];
                base_vels[j] = base_vels[j - 1];
                j -= 1;
            }
            base_notes[j] = key_note;
            base_vels[j] = key_vel;
        }

        let num_octaves = self.octaves.clamp(1, 3) as usize;
        let mut expanded_notes = [0u8; 48];
        let mut expanded_vels = [0u8; 48];
        let mut expanded_len = 0;

        for oct in 0..num_octaves {
            let transpose = (oct * 12) as u16;
            for i in 0..n {
                let note_transposed = base_notes[i] as u16 + transpose;
                if note_transposed <= 127 && expanded_len < 48 {
                    expanded_notes[expanded_len] = note_transposed as u8;
                    expanded_vels[expanded_len] = base_vels[i];
                    expanded_len += 1;
                }
            }
        }

        if expanded_len == 0 {
            self.sequence_len = 0;
            self.seq_index = 0;
            return;
        }

        match self.mode {
            ArpMode::Up => {
                for i in 0..expanded_len {
                    self.sequence[i] = expanded_notes[i];
                    self.sequence_velocities[i] = expanded_vels[i];
                }
                self.sequence_len = expanded_len;
            }
            ArpMode::Down => {
                for i in 0..expanded_len {
                    let rev_idx = expanded_len - 1 - i;
                    self.sequence[i] = expanded_notes[rev_idx];
                    self.sequence_velocities[i] = expanded_vels[rev_idx];
                }
                self.sequence_len = expanded_len;
            }
            ArpMode::UpDown => {
                if expanded_len <= 1 {
                    self.sequence[0] = expanded_notes[0];
                    self.sequence_velocities[0] = expanded_vels[0];
                    self.sequence_len = 1;
                } else {
                    let mut s = 0;
                    for i in 0..expanded_len {
                        self.sequence[s] = expanded_notes[i];
                        self.sequence_velocities[s] = expanded_vels[i];
                        s += 1;
                    }
                    for i in (1..expanded_len - 1).rev() {
                        if s < 48 {
                            self.sequence[s] = expanded_notes[i];
                            self.sequence_velocities[s] = expanded_vels[i];
                            s += 1;
                        }
                    }
                    self.sequence_len = s;
                }
            }
        }

        if self.seq_index >= self.sequence_len {
            self.seq_index = 0;
        }
    }

    /// Avanza una muestra de audio. Retorna un evento si corresponde disparar o soltar una nota.
    #[inline]
    pub fn advance_sample(&mut self, bpm: f32) -> Option<ArpOutputEvent> {
        if !self.enabled {
            return None;
        }

        // Si hay una nota sonando, contar muestras para apagarla al 80% (Gate)
        if let Some(active_note) = self.active_playing_note {
            self.note_sample_counter += 1;
            if self.note_sample_counter >= self.note_duration_samples {
                self.active_playing_note = None;
                return Some(ArpOutputEvent::NoteRelease { note: active_note });
            }
        }

        // Si no hay notas en la secuencia, no avanzar reloj de pasos
        if self.sequence_len == 0 {
            return None;
        }

        // Calcular muestras por tick a 96 ticks/negra
        let valid_bpm = bpm.clamp(30.0, 300.0);
        let samples_per_tick = (self.sample_rate_f as f64 * 60.0) / (valid_bpm as f64 * 96.0);

        self.sample_accumulator += 1.0;
        let mut triggered_event = None;

        while self.sample_accumulator >= samples_per_tick {
            self.sample_accumulator -= samples_per_tick;
            self.tick_accumulator += 1;

            let step_ticks = self.rate.ticks_per_step();
            if self.tick_accumulator >= step_ticks {
                self.tick_accumulator = 0;

                if self.sequence_len > 0 {
                    let note = self.sequence[self.seq_index];
                    let vel = self.sequence_velocities[self.seq_index];

                    // Avanzar índice circularmente
                    self.seq_index = (self.seq_index + 1) % self.sequence_len;

                    // Duración del paso en muestras
                    let step_duration_samples = (step_ticks as f64 * samples_per_tick) as usize;
                    // Articulación Gate al 80%
                    self.note_duration_samples = ((step_duration_samples as f64) * 0.80) as usize;
                    self.note_sample_counter = 0;
                    self.active_playing_note = Some(note);

                    triggered_event = Some(ArpOutputEvent::NoteTrigger { note, velocity: vel });
                }
            }
        }

        triggered_event
    }

    /// Detiene y limpia el estado de notas activas para evitar notas colgadas
    pub fn reset(&mut self) -> Option<u8> {
        let note_to_kill = self.active_playing_note;
        self.active_playing_note = None;
        self.tick_accumulator = 0;
        self.sample_accumulator = 0.0;
        self.note_sample_counter = 0;
        note_to_kill
    }

    pub fn set_mode(&mut self, mode: ArpMode) {
        self.mode = mode;
        self.rebuild_sequence();
    }

    pub fn set_rate(&mut self, rate: ArpRate) {
        self.rate = rate;
    }

    pub fn set_octaves(&mut self, octaves: u8) {
        self.octaves = octaves.clamp(1, 3);
        self.rebuild_sequence();
    }

    pub fn set_hold(&mut self, hold: bool) {
        self.hold = hold;
        if hold {
            self.hold_notes = self.held_notes;
            self.hold_velocities = self.held_velocities;
            self.hold_count = self.held_count;
        } else if self.held_count == 0 {
            self.hold_notes = [None; 16];
            self.hold_velocities = [0; 16];
            self.hold_count = 0;
            self.sequence_len = 0;
            self.seq_index = 0;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Option<u8> {
        self.enabled = enabled;
        if !enabled {
            self.sequence_len = 0;
            self.seq_index = 0;
            self.held_notes = [None; 16];
            self.held_count = 0;
            self.hold_notes = [None; 16];
            self.hold_count = 0;
            self.reset()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arp_up_sequence() {
        let mut arp = Arpeggiator::new(44100.0);
        arp.set_octaves(1);
        arp.set_mode(ArpMode::Up);

        arp.handle_note_on(67, 100);
        arp.handle_note_on(60, 100);
        arp.handle_note_on(64, 100);

        assert_eq!(arp.sequence_len, 3);
        assert_eq!(&arp.sequence[..3], &[60, 64, 67]);
    }

    #[test]
    fn test_arp_clock_advance() {
        let sample_rate = 48000.0;
        let mut arp = Arpeggiator::new(sample_rate);
        arp.enabled = true;
        arp.set_rate(ArpRate::Sixteenth); // 24 ticks
        // A 120 BPM: 1 tick = (48000 * 60) / (120 * 96) = 250 muestras.
        // 24 ticks = 6000 muestras.

        arp.handle_note_on(60, 100);

        let mut triggered_note = None;
        for _ in 0..6000 {
            if let Some(ArpOutputEvent::NoteTrigger { note, .. }) = arp.advance_sample(120.0) {
                triggered_note = Some(note);
                break;
            }
        }

        assert_eq!(triggered_note, Some(60));
    }
}
