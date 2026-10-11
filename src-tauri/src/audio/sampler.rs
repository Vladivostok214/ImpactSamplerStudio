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
        let (data_l, data_r) = if orig_rate == target_sample_rate || target_sample_rate == 0 {
            (orig_l, orig_r)
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
            (res_l, res_r)
        };

        let length = data_l.len();
        Ok(Self {
            name,
            sample_rate: target_sample_rate,
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
    pub position: usize,
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
            position: 0,
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
            gain: 0.85,
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
                slot.voice.position = 0;
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
        }
    }

    pub fn clear_sample(&mut self, pad_index: usize) {
        if pad_index < 8 {
            self.slots[pad_index].sample = None;
            self.slots[pad_index].voice.active = false;
        }
    }

    pub fn set_mode(&mut self, pad_index: usize, mode: PadPlaybackMode) {
        if pad_index < 8 {
            self.slots[pad_index].mode = mode;
        }
    }

    pub fn set_gain(&mut self, pad_index: usize, gain: f32) {
        if pad_index < 8 {
            self.slots[pad_index].gain = gain.clamp(0.0, 2.0);
        }
    }

    pub fn render_mix(&mut self, out_l: &mut [f32], out_r: &mut [f32], num_frames: usize) {
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
            let vel = slot.voice.velocity;
            let slot_gain = slot.gain;
            let fade_step = slot.voice.fade_step;

            let s_data_l = &sample_data.data_l;
            let s_data_r = &sample_data.data_r;

            for i in 0..num_frames {
                if slot.voice.position >= total_len {
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

                let effective_gain = vel * slot_gain * slot.voice.fade_volume;
                let pos = slot.voice.position;

                out_l[i] += s_data_l[pos] * effective_gain;
                out_r[i] += s_data_r[pos] * effective_gain;

                slot.voice.position += 1;
            }
        }
    }
}
