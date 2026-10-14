use cpal::traits::{DeviceTrait, HostTrait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let default_dev = host.default_output_device().and_then(|d| d.name().ok());
    println!("Default Device: {:?}", default_dev);

    let app_data = std::env::var("APPDATA").unwrap_or_default();
    let path = std::path::PathBuf::from(app_data)
        .join("com.impactsampler.studio")
        .join("soundfonts")
        .join("Yamaha_C7__Normalized_.sf2");

    if path.exists() {
        let mut file = std::fs::File::open(path)?;
        let sf = rustysynth::SoundFont::new(&mut file);
        match sf {
            Ok(s) => {
                println!("\nYamaha C7 Presets:");
                for (i, p) in s.get_presets().iter().enumerate() {
                    println!("  [{}] '{}' Bank: {} Patch: {}", i, p.get_name(), p.get_bank_number(), p.get_patch_number());
                }

                let sf_arc = std::sync::Arc::new(s);
                let settings = rustysynth::SynthesizerSettings::new(44100);

                let render_note = |bank: i32, patch: i32| -> Vec<f32> {
                    let mut synth = rustysynth::Synthesizer::new(&sf_arc, &settings).unwrap();
                    synth.process_midi_message(0, 0xB0, 0, bank);
                    synth.process_midi_message(0, 0xB0, 32, 0);
                    synth.process_midi_message(0, 0xC0, patch, 0);
                    synth.note_on(0, 60, 100);
                    let mut left = vec![0.0f32; 44100];
                    let mut right = vec![0.0f32; 44100];
                    synth.render(&mut left, &mut right);
                    left
                };

                let p0 = render_note(0, 0);
                let p1 = render_note(0, 1);
                let p2 = render_note(0, 2);
                let p3 = render_note(0, 3);

                let diff01: f32 = p0.iter().zip(p1.iter()).map(|(a, b)| (a - b).abs()).sum();
                let diff02: f32 = p0.iter().zip(p2.iter()).map(|(a, b)| (a - b).abs()).sum();
                let diff03: f32 = p0.iter().zip(p3.iter()).map(|(a, b)| (a - b).abs()).sum();

                println!("\nDiferencia acústica absoluta (suma de deltas de onda):");
                println!("  Preset 0 ('C7 Grand') vs Preset 1 ('C7 Dark'): {:.4}", diff01);
                println!("  Preset 0 ('C7 Grand') vs Preset 2 ('C7 Grand 88'): {:.4}", diff02);
                println!("  Preset 0 ('C7 Grand') vs Preset 3 ('C7 Bright 88'): {:.4}", diff03);
            }
            Err(e) => println!("Error loading C7: {:?}", e),
        }
    } else {
        println!("Path does not exist: {:?}", path);
    }
    Ok(())
}
