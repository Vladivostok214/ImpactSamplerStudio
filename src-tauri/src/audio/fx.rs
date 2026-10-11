use std::f32::consts::PI;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FxRouting {
    Keys,
    Both,
    Pads,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KnobTarget {
    Cutoff,
    Resonance,
    Drive,
    ReverbMix,
    DelayTime,
    DelayFeedback,
    DelayMix,
    ChorusMix,
    Pan,
}

impl KnobTarget {
    pub fn label(&self) -> &'static str {
        match self {
            KnobTarget::Cutoff => "CUTOFF",
            KnobTarget::Resonance => "RES",
            KnobTarget::Drive => "DRIVE",
            KnobTarget::ReverbMix => "REVERB",
            KnobTarget::DelayTime => "D-TIME",
            KnobTarget::DelayFeedback => "D-FDBK",
            KnobTarget::DelayMix => "D-MIX",
            KnobTarget::ChorusMix => "CHORUS",
            KnobTarget::Pan => "PAN",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            KnobTarget::Cutoff => "Filtro Cutoff (Paso Bajo)",
            KnobTarget::Resonance => "Filtro Resonancia (Q)",
            KnobTarget::Drive => "Warm Drive (Saturación)",
            KnobTarget::ReverbMix => "Reverb Mix (Espacialidad)",
            KnobTarget::DelayTime => "Delay Tiempo (Eco)",
            KnobTarget::DelayFeedback => "Delay Feedback (Repeticiones)",
            KnobTarget::DelayMix => "Delay Mix (Mezcla)",
            KnobTarget::ChorusMix => "Chorus Mix (Modulación)",
            KnobTarget::Pan => "Panorámica Estéreo (L/R)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnobMappingInfo {
    pub index: usize,
    pub target: KnobTarget,
    pub label: String,
    pub display_name: String,
    pub value: f32, // 0.0 a 1.0
}

// 1. Filtro State Variable Filter (SVF Chamberlin 12dB/oct)
#[derive(Clone)]
struct SvfFilter {
    low_l: f32,
    band_l: f32,
    low_r: f32,
    band_r: f32,
    cutoff_smooth: f32,
    res_smooth: f32,
}

impl SvfFilter {
    fn new() -> Self {
        Self {
            low_l: 0.0,
            band_l: 0.0,
            low_r: 0.0,
            band_r: 0.0,
            cutoff_smooth: 1.0,
            res_smooth: 0.0,
        }
    }

    fn process(&mut self, in_l: f32, in_r: f32, target_cutoff: f32, target_res: f32, sample_rate_f: f32) -> (f32, f32) {
        // Suavizado exponencial para cero zipper-noise
        self.cutoff_smooth += (target_cutoff - self.cutoff_smooth) * 0.005;
        self.res_smooth += (target_res - self.res_smooth) * 0.005;

        // Frecuencia de corte logarítmica: de 40 Hz a 18000 Hz
        let min_freq: f32 = 40.0;
        let max_freq: f32 = 18000.0;
        let cutoff_hz = min_freq * (max_freq / min_freq).powf(self.cutoff_smooth.clamp(0.01, 1.0));

        let f = (2.0 * (PI * (cutoff_hz / sample_rate_f)).sin()).min(0.85);
        let q = (1.0 - self.res_smooth * 0.88).max(0.12);

        // Canal Izquierdo
        let high_l = in_l - self.low_l - q * self.band_l;
        self.band_l += f * high_l;
        self.low_l += f * self.band_l;

        // Canal Derecho
        let high_r = in_r - self.low_r - q * self.band_r;
        self.band_r += f * high_r;
        self.low_r += f * self.band_r;

        (self.low_l, self.low_r)
    }
}

// 2. Delay Estéreo con buffer circular
#[derive(Clone)]
struct StereoDelay {
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    max_frames: usize,
    smooth_time_ms: f32,
    smooth_feedback: f32,
    smooth_mix: f32,
}

impl StereoDelay {
    fn new(sample_rate_f: f32) -> Self {
        let max_frames = (2.0 * sample_rate_f) as usize; // 2 segundos máx
        Self {
            buffer_l: vec![0.0; max_frames],
            buffer_r: vec![0.0; max_frames],
            write_pos: 0,
            max_frames,
            smooth_time_ms: 350.0,
            smooth_feedback: 0.35,
            smooth_mix: 0.0,
        }
    }

    fn process(&mut self, in_l: f32, in_r: f32, target_time_ms: f32, target_feedback: f32, target_mix: f32, sample_rate_f: f32) -> (f32, f32) {
        self.smooth_time_ms += (target_time_ms - self.smooth_time_ms) * 0.003;
        self.smooth_feedback += (target_feedback - self.smooth_feedback) * 0.005;
        self.smooth_mix += (target_mix - self.smooth_mix) * 0.005;

        let mix = self.smooth_mix;
        if mix <= 0.001 {
            self.buffer_l[self.write_pos] = in_l;
            self.buffer_r[self.write_pos] = in_r;
            self.write_pos = (self.write_pos + 1) % self.max_frames;
            return (in_l, in_r);
        }

        let delay_frames = (self.smooth_time_ms * 0.001 * sample_rate_f).clamp(10.0, (self.max_frames - 2) as f32);
        let read_pos_exact = (self.write_pos as f32 + self.max_frames as f32 - delay_frames) % (self.max_frames as f32);
        let read_idx0 = read_pos_exact.floor() as usize % self.max_frames;
        let read_idx1 = (read_idx0 + 1) % self.max_frames;
        let frac = read_pos_exact - read_idx0 as f32;

        let delayed_l = self.buffer_l[read_idx0] * (1.0 - frac) + self.buffer_l[read_idx1] * frac;
        let delayed_r = self.buffer_r[read_idx0] * (1.0 - frac) + self.buffer_r[read_idx1] * frac;

        let fb = self.smooth_feedback.clamp(0.0, 0.88);
        // Soft clipping en el feedback para saturación musical
        let fb_l = (delayed_l * fb).clamp(-1.0, 1.0);
        let fb_r = (delayed_r * fb).clamp(-1.0, 1.0);

        self.buffer_l[self.write_pos] = in_l + fb_l;
        self.buffer_r[self.write_pos] = in_r + fb_r;
        self.write_pos = (self.write_pos + 1) % self.max_frames;

        let out_l = in_l * (1.0 - mix * 0.5) + delayed_l * mix;
        let out_r = in_r * (1.0 - mix * 0.5) + delayed_r * mix;

        (out_l, out_r)
    }
}

// 3. Stereo Reverb (Freeverb / Schroeder: 8 comb + 4 allpass)
#[derive(Clone)]
struct FreeverbComb {
    buffer: Vec<f32>,
    pos: usize,
    filter_store: f32,
    size: usize,
}

impl FreeverbComb {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            filter_store: 0.0,
            size,
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.buffer[self.pos];
        self.filter_store = output * (1.0 - damp) + self.filter_store * damp;
        self.buffer[self.pos] = input + self.filter_store * feedback;
        self.pos = (self.pos + 1) % self.size;
        output
    }
}

#[derive(Clone)]
struct FreeverbAllpass {
    buffer: Vec<f32>,
    pos: usize,
    size: usize,
}

impl FreeverbAllpass {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            size,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.pos];
        let output = -input + buf_out;
        self.buffer[self.pos] = input + buf_out * 0.5;
        self.pos = (self.pos + 1) % self.size;
        output
    }
}

#[derive(Clone)]
struct StereoReverb {
    combs_l: Vec<FreeverbComb>,
    combs_r: Vec<FreeverbComb>,
    allpasses_l: Vec<FreeverbAllpass>,
    allpasses_r: Vec<FreeverbAllpass>,
    smooth_mix: f32,
}

impl StereoReverb {
    fn new(sample_rate_f: f32) -> Self {
        let scale = sample_rate_f / 44100.0;
        let comb_tunings_l = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        let allpass_tunings_l = [556, 441, 341, 225];
        let stereo_spread = 23;

        let combs_l = comb_tunings_l.iter().map(|&t| FreeverbComb::new((t as f32 * scale) as usize)).collect();
        let combs_r = comb_tunings_l.iter().map(|&t| FreeverbComb::new(((t + stereo_spread) as f32 * scale) as usize)).collect();
        let allpasses_l = allpass_tunings_l.iter().map(|&t| FreeverbAllpass::new((t as f32 * scale) as usize)).collect();
        let allpasses_r = allpass_tunings_l.iter().map(|&t| FreeverbAllpass::new(((t + stereo_spread) as f32 * scale) as usize)).collect();

        Self {
            combs_l,
            combs_r,
            allpasses_l,
            allpasses_r,
            smooth_mix: 0.0,
        }
    }

    fn process(&mut self, in_l: f32, in_r: f32, target_mix: f32) -> (f32, f32) {
        self.smooth_mix += (target_mix - self.smooth_mix) * 0.005;
        let mix = self.smooth_mix;
        if mix <= 0.001 {
            return (in_l, in_r);
        }

        let input = (in_l + in_r) * 0.015;
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        let feedback = 0.84;
        let damp = 0.2;

        for comb in self.combs_l.iter_mut() {
            out_l += comb.process(input, feedback, damp);
        }
        for comb in self.combs_r.iter_mut() {
            out_r += comb.process(input, feedback, damp);
        }

        for ap in self.allpasses_l.iter_mut() {
            out_l = ap.process(out_l);
        }
        for ap in self.allpasses_r.iter_mut() {
            out_r = ap.process(out_r);
        }

        let wet_gain = mix * 1.5;
        let dry_gain = 1.0 - mix * 0.3;

        (in_l * dry_gain + out_l * wet_gain, in_r * dry_gain + out_r * wet_gain)
    }
}

// 4. Stereo Chorus (Modulated Delay Line)
#[derive(Clone)]
struct StereoChorus {
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    pos: usize,
    lfo_phase: f32,
    smooth_mix: f32,
}

impl StereoChorus {
    fn new(sample_rate_f: f32) -> Self {
        let size = (0.05 * sample_rate_f) as usize; // 50ms buffer
        Self {
            buffer_l: vec![0.0; size],
            buffer_r: vec![0.0; size],
            pos: 0,
            lfo_phase: 0.0,
            smooth_mix: 0.0,
        }
    }

    fn process(&mut self, in_l: f32, in_r: f32, target_mix: f32, sample_rate_f: f32) -> (f32, f32) {
        self.smooth_mix += (target_mix - self.smooth_mix) * 0.005;
        let mix = self.smooth_mix;
        if mix <= 0.001 {
            return (in_l, in_r);
        }

        let size = self.buffer_l.len();
        self.buffer_l[self.pos] = in_l;
        self.buffer_r[self.pos] = in_r;

        // LFO a 0.85 Hz
        let lfo_inc = (0.85 * 2.0 * PI) / sample_rate_f;
        self.lfo_phase += lfo_inc;
        if self.lfo_phase > 2.0 * PI {
            self.lfo_phase -= 2.0 * PI;
        }

        // LFO L y LFO R desfasados 90 grados
        let mod_l = (self.lfo_phase.sin() * 0.5 + 0.5) * 12.0 + 8.0; // 8ms a 20ms
        let mod_r = ((self.lfo_phase + PI * 0.5).sin() * 0.5 + 0.5) * 12.0 + 8.0;

        let delay_frames_l = (mod_l * 0.001 * sample_rate_f).clamp(1.0, (size - 1) as f32);
        let delay_frames_r = (mod_r * 0.001 * sample_rate_f).clamp(1.0, (size - 1) as f32);

        let read_pos_l = (self.pos as f32 + size as f32 - delay_frames_l) % (size as f32);
        let read_pos_r = (self.pos as f32 + size as f32 - delay_frames_r) % (size as f32);

        let r_l_idx = read_pos_l.floor() as usize % size;
        let r_r_idx = read_pos_r.floor() as usize % size;

        let ch_l = self.buffer_l[r_l_idx];
        let ch_r = self.buffer_r[r_r_idx];

        self.pos = (self.pos + 1) % size;

        (in_l + ch_l * mix * 0.7, in_r + ch_r * mix * 0.7)
    }
}

// Rack DSP Principal
#[derive(Clone)]
pub struct AudioFxRack {
    sample_rate_f: f32,
    svf: SvfFilter,
    delay: StereoDelay,
    reverb: StereoReverb,
    chorus: StereoChorus,
    // Valores de parámetros actuales (0.0 a 1.0)
    pub cutoff_val: f32,
    pub resonance_val: f32,
    pub drive_val: f32,
    pub reverb_mix_val: f32,
    pub delay_time_val: f32,
    pub delay_feedback_val: f32,
    pub delay_mix_val: f32,
    pub chorus_mix_val: f32,
    pub pan_val: f32,
    // Mapeo actual de los 8 knobs físicos
    pub knob_mappings: [KnobTarget; 8],
    pub knob_values: [f32; 8],
}

impl AudioFxRack {
    pub fn new(sample_rate_f: f32) -> Self {
        // Disposición solicitada por el usuario:
        // Superiores (K1..K4): Cutoff, Resonance, Drive, Reverb
        // Inferiores (K5..K8): Delay Time, Delay Feedback, Delay Mix, Pan
        let knob_mappings = [
            KnobTarget::Cutoff,
            KnobTarget::Resonance,
            KnobTarget::Drive,
            KnobTarget::ReverbMix,
            KnobTarget::DelayTime,
            KnobTarget::DelayFeedback,
            KnobTarget::DelayMix,
            KnobTarget::Pan,
        ];

        let mut rack = Self {
            sample_rate_f,
            svf: SvfFilter::new(),
            delay: StereoDelay::new(sample_rate_f),
            reverb: StereoReverb::new(sample_rate_f),
            chorus: StereoChorus::new(sample_rate_f),
            cutoff_val: 1.0,       // Abierto por defecto
            resonance_val: 0.1,    // Resonancia suave
            drive_val: 0.0,        // Sin distorsión
            reverb_mix_val: 0.25,  // Reverb ligera
            delay_time_val: 0.35,  // ~350ms
            delay_feedback_val: 0.35, // Repeticiones moderadas
            delay_mix_val: 0.0,    // Delay apagado por defecto
            chorus_mix_val: 0.0,   // Chorus apagado
            pan_val: 0.5,          // Centro (0.5 = 0.0 pan)
            knob_mappings,
            knob_values: [1.0, 0.1, 0.0, 0.25, 0.35, 0.35, 0.0, 0.5],
        };
        rack.sync_knob_values();
        rack
    }

    pub fn set_knob_value(&mut self, knob_idx: usize, value: f32) {
        if knob_idx < 8 {
            let v = value.clamp(0.0, 1.0);
            self.knob_values[knob_idx] = v;
            let target = self.knob_mappings[knob_idx];
            self.apply_target_value(target, v);
        }
    }

    pub fn set_knob_mapping(&mut self, knob_idx: usize, target: KnobTarget) {
        if knob_idx < 8 {
            self.knob_mappings[knob_idx] = target;
            let current_val = self.get_target_value(target);
            self.knob_values[knob_idx] = current_val;
        }
    }

    fn apply_target_value(&mut self, target: KnobTarget, v: f32) {
        match target {
            KnobTarget::Cutoff => self.cutoff_val = v,
            KnobTarget::Resonance => self.resonance_val = v,
            KnobTarget::Drive => self.drive_val = v,
            KnobTarget::ReverbMix => self.reverb_mix_val = v,
            KnobTarget::DelayTime => self.delay_time_val = v,
            KnobTarget::DelayFeedback => self.delay_feedback_val = v,
            KnobTarget::DelayMix => self.delay_mix_val = v,
            KnobTarget::ChorusMix => self.chorus_mix_val = v,
            KnobTarget::Pan => self.pan_val = v,
        }
    }

    fn get_target_value(&self, target: KnobTarget) -> f32 {
        match target {
            KnobTarget::Cutoff => self.cutoff_val,
            KnobTarget::Resonance => self.resonance_val,
            KnobTarget::Drive => self.drive_val,
            KnobTarget::ReverbMix => self.reverb_mix_val,
            KnobTarget::DelayTime => self.delay_time_val,
            KnobTarget::DelayFeedback => self.delay_feedback_val,
            KnobTarget::DelayMix => self.delay_mix_val,
            KnobTarget::ChorusMix => self.chorus_mix_val,
            KnobTarget::Pan => self.pan_val,
        }
    }

    fn sync_knob_values(&mut self) {
        for i in 0..8 {
            let target = self.knob_mappings[i];
            self.knob_values[i] = self.get_target_value(target);
        }
    }

    pub fn get_mappings_info(&self) -> Vec<KnobMappingInfo> {
        self.knob_mappings
            .iter()
            .enumerate()
            .map(|(i, &target)| KnobMappingInfo {
                index: i,
                target,
                label: target.label().to_string(),
                display_name: target.display_name().to_string(),
                value: self.knob_values[i],
            })
            .collect()
    }

    pub fn process_stereo_frame(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        // 1. Filtro SVF (Cutoff / Resonancia)
        let (flt_l, flt_r) = self.svf.process(l, r, self.cutoff_val, self.resonance_val, self.sample_rate_f);
        l = flt_l;
        r = flt_r;

        // 2. Drive / Saturación cálida
        if self.drive_val > 0.01 {
            let drive_gain = 1.0 + self.drive_val * 3.5;
            let dl = l * drive_gain;
            let dr = r * drive_gain;
            // Soft clipping no-lineal suave
            l = (dl / (1.0 + dl.abs() * 0.65)).clamp(-1.0, 1.0);
            r = (dr / (1.0 + dr.abs() * 0.65)).clamp(-1.0, 1.0);
        }

        // 3. Stereo Chorus
        if self.chorus_mix_val > 0.01 {
            let (ch_l, ch_r) = self.chorus.process(l, r, self.chorus_mix_val, self.sample_rate_f);
            l = ch_l;
            r = ch_r;
        }

        // 4. Stereo Delay (40ms a 1000ms)
        let delay_ms = 40.0 + self.delay_time_val * 960.0;
        let (dly_l, dly_r) = self.delay.process(l, r, delay_ms, self.delay_feedback_val, self.delay_mix_val, self.sample_rate_f);
        l = dly_l;
        r = dly_r;

        // 5. Stereo Reverb
        let (rev_l, rev_r) = self.reverb.process(l, r, self.reverb_mix_val);
        l = rev_l;
        r = rev_r;

        // 6. Panorámica Estéreo (Ley de potencia constante)
        let pan_norm = (self.pan_val * 2.0 - 1.0).clamp(-1.0, 1.0); // -1.0 (izq) a 1.0 (der)
        let angle = (pan_norm + 1.0) * (PI * 0.25);
        let gain_l = angle.cos();
        let gain_r = angle.sin();

        (l * gain_l, r * gain_r)
    }
}
