use std::f32::consts::PI;
use std::path::Path;
use hound::{WavSpec, WavWriter, SampleFormat};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = if Path::new("samples").exists() {
        Path::new("samples")
    } else {
        Path::new("../samples")
    };

    let spec = WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    // 1. Kick (Punchy 808/909 style)
    {
        let path = out_dir.join("01_Kick.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.45 * 44100.0) as usize;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let env = (-t * 12.0).exp();
            let freq = 45.0 + 110.0 * (-t * 35.0).exp();
            let click = if t < 0.005 { (1.0 - t / 0.005) * 0.4 } else { 0.0 };
            let s = ((2.0 * PI * freq * t).sin() * env + click).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    // 2. Snare (Body + noise burst)
    {
        let path = out_dir.join("02_Snare.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.28 * 44100.0) as usize;
        let mut rng_seed: u32 = 12345;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let tone_env = (-t * 22.0).exp();
            let tone = (2.0 * PI * 185.0 * t).sin() * tone_env * 0.45;
            let noise_env = (-t * 14.0).exp();
            rng_seed = rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = ((rng_seed as f32 / u32::MAX as f32) * 2.0 - 1.0) * noise_env * 0.55;
            let s = (tone + noise).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    // 3. Clap (Flam transients + decay)
    {
        let path = out_dir.join("03_Clap.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.35 * 44100.0) as usize;
        let mut rng_seed: u32 = 67890;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            rng_seed = rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = (rng_seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let flam = if t < 0.012 {
                (-t * 150.0).exp()
            } else if t < 0.024 {
                (-(t - 0.012) * 150.0).exp()
            } else if t < 0.036 {
                (-(t - 0.024) * 150.0).exp()
            } else {
                (-(t - 0.036) * 18.0).exp()
            };
            let s = (noise * flam * 0.75).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    // 4. Closed Hat (Metallic crisp)
    {
        let path = out_dir.join("04_Closed_Hat.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.09 * 44100.0) as usize;
        let mut rng_seed: u32 = 98765;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let env = (-t * 60.0).exp();
            rng_seed = rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = (rng_seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let metallic = (2.0 * PI * 8000.0 * t).sin() * 0.3 + (2.0 * PI * 12000.0 * t).sin() * 0.3;
            let s = ((noise * 0.6 + metallic * 0.4) * env * 0.7).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    // 5. Open Hat (Longer metallic sizzle)
    {
        let path = out_dir.join("05_Open_Hat.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.45 * 44100.0) as usize;
        let mut rng_seed: u32 = 45678;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let env = (-t * 7.5).exp();
            rng_seed = rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = (rng_seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let metallic = (2.0 * PI * 7500.0 * t).sin() * 0.3 + (2.0 * PI * 10500.0 * t).sin() * 0.3;
            let s = ((noise * 0.6 + metallic * 0.4) * env * 0.65).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    // 6. Low Tom (85 Hz pitch envelope)
    {
        let path = out_dir.join("06_Low_Tom.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.35 * 44100.0) as usize;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let env = (-t * 9.0).exp();
            let freq = 75.0 + 90.0 * (-t * 30.0).exp();
            let s = ((2.0 * PI * freq * t).sin() * env * 0.75).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    // 7. High Tom (140 Hz pitch envelope)
    {
        let path = out_dir.join("07_High_Tom.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.30 * 44100.0) as usize;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let env = (-t * 11.0).exp();
            let freq = 130.0 + 120.0 * (-t * 30.0).exp();
            let s = ((2.0 * PI * freq * t).sin() * env * 0.75).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    // 8. Percussion / Rimshot (Crisp click)
    {
        let path = out_dir.join("08_Perc.wav");
        let mut writer = WavWriter::create(path, spec)?;
        let num_samples = (0.12 * 44100.0) as usize;
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let env = (-t * 40.0).exp();
            let tone = (2.0 * PI * 850.0 * t).sin() * 0.6 + (2.0 * PI * 1700.0 * t).sin() * 0.3;
            let s = (tone * env * 0.8).clamp(-1.0, 1.0);
            let s_i16 = (s * 32000.0) as i16;
            writer.write_sample(s_i16)?;
            writer.write_sample(s_i16)?;
        }
        writer.finalize()?;
    }

    println!("8 factory samples successfully generated in {:?}", out_dir);
    Ok(())
}
