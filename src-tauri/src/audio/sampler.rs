use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use hound::{SampleFormat, WavReader};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PadPlaybackMode {
    OneShot,
    Gate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PadEngineMode {
    WavSampler,
    DrumKitSf2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PadSlotInfo {
    pub index: usize,
    pub pad_number: usize,
    pub sample_name: Option<String>,
    pub mode: PadPlaybackMode,
    pub gain: f32,
    pub has_sample: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PadSampleData {
    pub name: String,
    pub sample_rate: u32,
    pub data_l: Vec<f32>,
    pub data_r: Vec<f32>,
    pub length: usize,
}

impl PadSampleData {
    pub fn from_wav_reader<R: std::io::Read + std::io::Seek>(
        mut reader: WavReader<R>,
        name: String,
        target_sample_rate: u32,
    ) -> Result<Self, String> {
        let spec = reader.spec();
        let channels = spec.channels as usize;
        let orig_rate = spec.sample_rate;

        if channels == 0 {
            return Err("El archivo WAV no tiene canales de audio".to_string());
        }

        let raw_samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
            (SampleFormat::Float, _) => {
                reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
            }
            (SampleFormat::Int, 8) => {
                reader.samples::<i8>()
                    .map(|s| s.unwrap_or(0) as f32 / 128.0)
                    .collect()
            }
            (SampleFormat::Int, 16) => {
                reader.samples::<i16>()
                    .map(|s| s.unwrap_or(0) as f32 / 32768.0)
                    .collect()
            }
            (SampleFormat::Int, 24) => {
                reader.samples::<i32>()
                    .map(|s| s.unwrap_or(0) as f32 / 8388608.0)
                    .collect()
            }
            (SampleFormat::Int, 32) => {
                reader.samples::<i32>()
                    .map(|s| s.unwrap_or(0) as f32 / 2147483648.0)
                    .collect()
            }
            _ => {
                return Err(format!(
                    "Formato WAV no soportado: {:?}, {} bits",
                    spec.sample_format, spec.bits_per_sample
                ));
            }
        };

        let orig_frames = raw_samples.len() / channels;
        if orig_frames == 0 {
            return Err("El archivo WAV está vacío".to_string());
        }

        // Desentrelazar a Left y Right
        let mut orig_l = Vec::with_capacity(orig_frames);
        let mut orig_r = Vec::with_capacity(orig_frames);

        if channels == 1 {
            for &s in &raw_samples {
                orig_l.push(s);
                orig_r.push(s);
            }
        } else {
            for frame in raw_samples.chunks(channels) {
                orig_l.push(frame[0]);
                orig_r.push(frame[1]);
            }
        }

        // Resamplear si el sample rate difiere del target
        let (data_l, data_r, effective_sample_rate) = if orig_rate == target_sample_rate || target_sample_rate == 0 {
            (orig_l, orig_r, orig_rate)
        } else {
            let ratio = orig_rate as f64 / target_sample_rate as f64;
            let target_len = ((orig_frames as f64) / ratio).round() as usize;
            let mut res_l = Vec::with_capacity(target_len);
            let mut res_r = Vec::with_capacity(target_len);

            for i in 0..target_len {
                let src_pos = i as f64 * ratio;
                let idx0 = src_pos.floor() as usize;
                let frac = (src_pos - idx0 as f64) as f32;

                if idx0 + 1 < orig_frames {
                    res_l.push(orig_l[idx0] * (1.0 - frac) + orig_l[idx0 + 1] * frac);
                    res_r.push(orig_r[idx0] * (1.0 - frac) + orig_r[idx0 + 1] * frac);
                } else if idx0 < orig_frames {
                    res_l.push(orig_l[idx0]);
                    res_r.push(orig_r[idx0]);
                } else {
                    res_l.push(0.0);
                    res_r.push(0.0);
                }
            }
            (res_l, res_r, target_sample_rate)
        };

        let length = data_l.len();
        Ok(Self {
            name,
            sample_rate: effective_sample_rate,
            data_l,
            data_r,
            length,
        })
    }

    pub fn from_file(path: &Path, target_sample_rate: u32) -> Result<Self, String> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sample")
            .to_string();
        let reader = WavReader::open(path)
            .map_err(|e| format!("Error abriendo {:?}: {}", path, e))?;
        Self::from_wav_reader(reader, name, target_sample_rate)
    }

    #[allow(dead_code)]
    pub fn from_bytes(name: &str, bytes: &[u8], target_sample_rate: u32) -> Result<Self, String> {
        let cursor = Cursor::new(bytes);
        let reader = WavReader::new(cursor)
            .map_err(|e| format!("Error decodificando WAV: {}", e))?;
        Self::from_wav_reader(reader, name.to_string(), target_sample_rate)
    }
}

#[derive(Clone)]
pub struct PadVoice {
    pub position: f64,
    pub velocity: f32,
    pub active: bool,
    pub releasing: bool,
    pub fade_volume: f32,
    pub fade_step: f32,
}

impl PadVoice {
    pub fn new(sample_rate_f: f32) -> Self {
        // Micro-fadeout de 5ms para evitar cualquier artefacto DC al soltar en modo Gate
        let fade_samples = (0.005 * sample_rate_f).max(32.0);
        Self {
            position: 0.0,
            velocity: 0.0,
            active: false,
            releasing: false,
            fade_volume: 1.0,
            fade_step: 1.0 / fade_samples,
        }
    }
}

pub struct PadSlot {
    pub sample: Option<Arc<PadSampleData>>,
    pub mode: PadPlaybackMode,
    pub gain: f32,
    pub voice: PadVoice,
}

impl PadSlot {
    pub fn new(sample_rate_f: f32) -> Self {
        Self {
            sample: None,
            mode: PadPlaybackMode::OneShot,
            gain: 64.0 / 127.0, // 50% de volumen nominal de salida
            voice: PadVoice::new(sample_rate_f),
        }
    }
}

#[allow(dead_code)]
pub struct PadSampler {
    pub slots: [PadSlot; 8],
    pub sample_rate_f: f32,
}

#[allow(dead_code)]
impl PadSampler {
    pub fn new(sample_rate_f: f32) -> Self {
        Self {
            slots: std::array::from_fn(|_| PadSlot::new(sample_rate_f)),
            sample_rate_f,
        }
    }

    pub fn update_sample_rate(&mut self, new_rate: f32) {
        self.sample_rate_f = new_rate;
        let fade_samples = (0.005 * new_rate).max(32.0);
        for slot in self.slots.iter_mut() {
            slot.voice.fade_step = 1.0 / fade_samples;
        }
    }

    pub fn trigger(&mut self, pad_index: usize, velocity: f32) {
        if pad_index < 8 {
            let slot = &mut self.slots[pad_index];
            if slot.sample.is_some() {
                slot.voice.position = 0.0;
                slot.voice.velocity = velocity.clamp(0.0, 1.0);
                slot.voice.active = true;
                slot.voice.releasing = false;
                slot.voice.fade_volume = 1.0;
            }
        }
    }

    pub fn release(&mut self, pad_index: usize) {
        if pad_index < 8 {
            let slot = &mut self.slots[pad_index];
            if slot.voice.active {
                match slot.mode {
                    PadPlaybackMode::OneShot => {
                        // One-Shot ignora el release y suena hasta el final
                    }
                    PadPlaybackMode::Gate => {
                        // Gate inicia micro-fadeout
                        slot.voice.releasing = true;
                    }
                }
            }
        }
    }

    pub fn load_sample(&mut self, pad_index: usize, sample: Arc<PadSampleData>) {
        if pad_index < 8 {
            self.slots[pad_index].sample = Some(sample);
            self.slots[pad_index].voice.active = false;
            self.slots[pad_index].voice.position = 0.0;
        }
    }

    pub fn clear_sample(&mut self, pad_index: usize) {
        if pad_index < 8 {
            self.slots[pad_index].sample = None;
            self.slots[pad_index].voice.active = false;
            self.slots[pad_index].voice.position = 0.0;
        }
    }

    pub fn set_mode(&mut self, pad_index: usize, mode: PadPlaybackMode) {
        if pad_index < 8 {
            self.slots[pad_index].mode = mode;
        }
    }

    pub fn set_gain(&mut self, pad_index: usize, gain: f32) {
        if pad_index < 8 {
            self.slots[pad_index].gain = gain.clamp(0.0, 1.0);
        }
    }

    pub fn render_mix(&mut self, out_l: &mut [f32], out_r: &mut [f32], num_frames: usize) {
        let max_frames = num_frames.min(out_l.len()).min(out_r.len());
        for slot in self.slots.iter_mut() {
            if !slot.voice.active {
                continue;
            }

            let sample_data = match &slot.sample {
                Some(s) => s,
                None => {
                    slot.voice.active = false;
                    continue;
                }
            };

            let total_len = sample_data.length;
            if total_len == 0 {
                slot.voice.active = false;
                continue;
            }

            let vel = slot.voice.velocity;
            let slot_gain = slot.gain;
            let fade_step = slot.voice.fade_step;

            let s_data_l = &sample_data.data_l;
            let s_data_r = &sample_data.data_r;

            // Incremento fraccionario (A05): step = sample.sample_rate / engine_sample_rate
            // Garantiza que los samples mantengan su duración y afinación exactas independientemente
            // de la frecuencia de muestreo del dispositivo de audio (44.1 kHz, 48 kHz, 96 kHz)
            // sin remuestrear destructivamente los buffers PCM en memoria.
            let sample_rate = if sample_data.sample_rate > 0 {
                sample_data.sample_rate as f64
            } else if self.sample_rate_f > 0.0 {
                self.sample_rate_f as f64
            } else {
                48000.0
            };
            let engine_rate = if self.sample_rate_f > 0.0 {
                self.sample_rate_f as f64
            } else {
                48000.0
            };
            let step = (sample_rate / engine_rate).max(1e-6);

            for i in 0..max_frames {
                let pos = slot.voice.position;
                let idx0 = pos.floor() as usize;

                if idx0 >= total_len {
                    slot.voice.active = false;
                    break;
                }

                if slot.voice.releasing {
                    slot.voice.fade_volume -= fade_step;
                    if slot.voice.fade_volume <= 0.0 {
                        slot.voice.fade_volume = 0.0;
                        slot.voice.active = false;
                        break;
                    }
                }

                let frac = (pos - idx0 as f64) as f32;
                let (s_l, s_r) = if idx0 + 1 < total_len {
                    (
                        s_data_l[idx0] * (1.0 - frac) + s_data_l[idx0 + 1] * frac,
                        s_data_r[idx0] * (1.0 - frac) + s_data_r[idx0 + 1] * frac,
                    )
                } else {
                    (
                        s_data_l[idx0] * (1.0 - frac),
                        s_data_r[idx0] * (1.0 - frac),
                    )
                };

                let effective_gain = vel * slot_gain * slot.voice.fade_volume;
                out_l[i] += s_l * effective_gain;
                out_r[i] += s_r * effective_gain;

                slot.voice.position += step;
                if slot.voice.position >= total_len as f64 {
                    slot.voice.active = false;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a05_fractional_playback_duration_preservation() {
        // Crear un sample de 1.0 segundo grabado a 44100 Hz (44100 frames)
        let sample_sr = 44100u32;
        let num_samples = 44100usize;
        let test_sample = Arc::new(PadSampleData {
            name: "Sine440_44k.wav".to_string(),
            sample_rate: sample_sr,
            data_l: vec![0.5f32; num_samples],
            data_r: vec![0.5f32; num_samples],
            length: num_samples,
        });

        // 1. Probar en un motor a 44100 Hz (step = 1.0)
        {
            let mut sampler = PadSampler::new(44100.0);
            sampler.load_sample(0, test_sample.clone());
            sampler.trigger(0, 1.0);

            let mut out_l = vec![0.0f32; 44100];
            let mut out_r = vec![0.0f32; 44100];
            sampler.render_mix(&mut out_l, &mut out_r, 44100);

            // Al cumplirse exactamente 1 segundo (44100 frames), el sample debe haber concluido
            assert!(!sampler.slots[0].voice.active, "A 44.1 kHz debe finalizar en 44100 frames");
            assert!(out_l[0] > 0.0);
            assert!(out_l[44099] > 0.0);
        }

        // 2. Probar en un motor a 48000 Hz (step = 44100 / 48000 = 0.91875)
        {
            let mut sampler = PadSampler::new(48000.0);
            sampler.load_sample(0, test_sample.clone());
            sampler.trigger(0, 1.0);

            // Renderizar 47999 frames: la voz aún debe seguir activa
            let mut out_l = vec![0.0f32; 47999];
            let mut out_r = vec![0.0f32; 47999];
            sampler.render_mix(&mut out_l, &mut out_r, 47999);
            assert!(sampler.slots[0].voice.active, "A 48 kHz aún debe sonar en el frame 47999");

            // Renderizar los 2 frames restantes: en el frame 48000 debe finalizar
            let mut rem_l = vec![0.0f32; 2];
            let mut rem_r = vec![0.0f32; 2];
            sampler.render_mix(&mut rem_l, &mut rem_r, 2);
            assert!(!sampler.slots[0].voice.active, "A 48 kHz debe finalizar en exactamente 48000 frames (1.0s)");
        }

        // 3. Probar en un motor a 96000 Hz (step = 44100 / 96000 = 0.459375)
        {
            let mut sampler = PadSampler::new(96000.0);
            sampler.load_sample(0, test_sample.clone());
            sampler.trigger(0, 1.0);

            // Renderizar 95999 frames
            let mut out_l = vec![0.0f32; 95999];
            let mut out_r = vec![0.0f32; 95999];
            sampler.render_mix(&mut out_l, &mut out_r, 95999);
            assert!(sampler.slots[0].voice.active, "A 96 kHz aún debe sonar en el frame 95999");

            // Renderizar los 2 frames restantes
            let mut rem_l = vec![0.0f32; 2];
            let mut rem_r = vec![0.0f32; 2];
            sampler.render_mix(&mut rem_l, &mut rem_r, 2);
            assert!(!sampler.slots[0].voice.active, "A 96 kHz debe finalizar en exactamente 96000 frames (1.0s)");
        }
    }

    #[test]
    fn test_a05_fractional_playback_pitch_preservation() {
        // Generar un tono senoidal puro de 440 Hz a 48000 Hz
        let sample_sr = 48000.0f32;
        let freq = 440.0f64;
        let num_samples = 48000;
        let mut sine_data = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f64 / sample_sr as f64;
            let val = (2.0 * std::f64::consts::PI * freq * t).sin() as f32;
            sine_data.push(val);
        }

        let sine_sample = Arc::new(PadSampleData {
            name: "Sine440.wav".to_string(),
            sample_rate: 48000,
            data_l: sine_data.clone(),
            data_r: sine_data,
            length: num_samples,
        });

        // Reproducir a 96000 Hz en un DAC de alta fidelidad
        let mut sampler = PadSampler::new(96000.0);
        sampler.load_sample(0, sine_sample);
        sampler.set_gain(0, 1.0);
        sampler.trigger(0, 1.0);

        let mut out_l = vec![0.0f32; 960];
        let mut out_r = vec![0.0f32; 960];
        sampler.render_mix(&mut out_l, &mut out_r, 960);

        // Verificar que en cualquier frame de tiempo `t = n / 96000`, la muestra coincida
        // con la función continua senoidal original de 440 Hz
        for (n, &sample) in out_l.iter().enumerate() {
            let t = n as f64 / 96000.0;
            let expected = (2.0 * std::f64::consts::PI * freq * t).sin() as f32;
            // Con interpolación lineal a 96 kHz, el error relativo es inferior a 0.005
            let diff = (sample - expected).abs();
            assert!(
                diff < 0.005,
                "La afinación a 96 kHz debe coincidir con la senoide de 440 Hz a nivel de muestra: diff={} en n={}",
                diff,
                n
            );
        }
    }

    #[test]
    fn test_a05_end_of_sample_smooth_decay_no_oob() {
        let tiny_sample = Arc::new(PadSampleData {
            name: "Tiny.wav".to_string(),
            sample_rate: 44100,
            data_l: vec![0.2, 0.4, 0.6, 0.8, 1.0],
            data_r: vec![0.2, 0.4, 0.6, 0.8, 1.0],
            length: 5,
        });

        let mut sampler = PadSampler::new(48000.0);
        sampler.load_sample(0, tiny_sample);
        sampler.trigger(0, 1.0);

        let mut out_l = [0.0f32; 32];
        let mut out_r = [0.0f32; 32];
        sampler.render_mix(&mut out_l, &mut out_r, 32);

        assert!(!sampler.slots[0].voice.active, "La voz debe desactivarse limpiamente al terminar el sample");
    }

    #[test]
    fn test_a05_voice_completion_short_samples_and_fractional_steps() {
        // Verificar exhaustivamente los casos solicitados:
        // - step < 1.0, step == 1.0, step > 1.0
        // - samples de 1, 2 y 5 frames
        // Comprobar que ningún frame válido se descarta prematuramente

        let test_cases = [
            // (longitud_sample, step, frames_esperados)
            (1, 0.5, 2),  // 1 frame a mitad de velocidad = 2 frames
            (1, 1.0, 1),  // 1 frame a velocidad normal = 1 frame
            (1, 2.0, 1),  // 1 frame a doble velocidad = 1 frame
            (2, 0.5, 4),  // 2 frames a mitad de velocidad = 4 frames
            (2, 1.0, 2),  // 2 frames a velocidad normal = 2 frames
            (2, 1.5, 2),  // 2 / 1.5 = 1.33 => 2 frames
            (5, 0.7, 8),  // 5 / 0.7 = 7.14 => 8 frames
            (5, 1.0, 5),  // 5 / 1.0 = 5 frames
            (5, 1.3, 4),  // 5 / 1.3 = 3.84 => 4 frames
            (5, 2.5, 2),  // 5 / 2.5 = 2 frames
        ];

        for &(len, step, expected_frames) in &test_cases {
            let sample_data = vec![1.0f32; len];
            // Fijar sample_rate y engine_rate para obtener exactamente el step deseado
            let engine_rate = 10000.0f32;
            let sample_rate = (engine_rate as f64 * step).round() as u32;

            let sample = Arc::new(PadSampleData {
                name: format!("Len{}_Step{}.wav", len, step),
                sample_rate,
                data_l: sample_data.clone(),
                data_r: sample_data,
                length: len,
            });

            // 1. Probar renderizando frame por frame (validación de límites en tiempo real)
            {
                let mut sampler = PadSampler::new(engine_rate);
                sampler.load_sample(0, sample.clone());
                sampler.set_gain(0, 1.0);
                sampler.trigger(0, 1.0);

                let mut frames_rendered = 0;
                for _ in 0..100 {
                    if !sampler.slots[0].voice.active {
                        break;
                    }
                    let mut out_l = [0.0f32; 1];
                    let mut out_r = [0.0f32; 1];
                    sampler.render_mix(&mut out_l, &mut out_r, 1);
                    assert!(
                        out_l[0] > 0.0,
                        "El frame {} no debe ser descartado ni silenciado (len={}, step={})",
                        frames_rendered,
                        len,
                        step
                    );
                    frames_rendered += 1;
                }

                assert_eq!(
                    frames_rendered, expected_frames,
                    "Discrepancia en frames renderizados para len={}, step={}",
                    len, step
                );
                assert!(
                    !sampler.slots[0].voice.active,
                    "La voz debe quedar inactiva tras renderizar todos los frames para len={}, step={}",
                    len, step
                );
            }

            // 2. Probar renderizando en un único bloque grande (validación de bloque completo)
            {
                let mut sampler = PadSampler::new(engine_rate);
                sampler.load_sample(0, sample);
                sampler.set_gain(0, 1.0);
                sampler.trigger(0, 1.0);

                let mut out_l = vec![0.0f32; expected_frames + 4];
                let mut out_r = vec![0.0f32; expected_frames + 4];
                sampler.render_mix(&mut out_l, &mut out_r, expected_frames + 4);

                // Comprobar que exactamente `expected_frames` son no nulos
                for f in 0..expected_frames {
                    assert!(
                        out_l[f] > 0.0,
                        "El frame {} en bloque no debe ser cero (len={}, step={})",
                        f,
                        len,
                        step
                    );
                }
                for f in expected_frames..out_l.len() {
                    assert_eq!(
                        out_l[f], 0.0,
                        "El frame {} más allá del fin debe ser cero (len={}, step={})",
                        f,
                        len,
                        step
                    );
                }
                assert!(!sampler.slots[0].voice.active);
            }
        }
    }
}
