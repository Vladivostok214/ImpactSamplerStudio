use cpal::traits::{DeviceTrait, HostTrait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let default_dev = host.default_output_device().and_then(|d| d.name().ok());
    println!("Default Device: {:?}", default_dev);

    let path = if std::path::Path::new("soundfonts/Yamaha_RX7_Drums.sf2").exists() {
        std::path::PathBuf::from("soundfonts/Yamaha_RX7_Drums.sf2")
    } else {
        std::path::PathBuf::from("../soundfonts/Yamaha_RX7_Drums.sf2")
    };
    if path.exists() {
        let mut file = std::fs::File::open(path)?;
        let sf = rustysynth::SoundFont::new(&mut file);
        match sf {
            Ok(s) => {
                println!("\nYamaha RX7 Presets:");
                for (i, p) in s.get_presets().iter().enumerate() {
                    println!("  [{}] '{}' Bank: {} Patch: {}", i, p.get_name(), p.get_bank_number(), p.get_patch_number());
                }
            }
            Err(e) => println!("Error loading RX7: {:?}", e),
        }
    } else {
        println!("Path soundfonts/Yamaha_RX7_Drums.sf2 does not exist");
    }
    Ok(())
}
