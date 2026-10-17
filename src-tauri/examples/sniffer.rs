use std::error::Error;
use std::io::{self, BufRead};
use midir::{Ignore, MidiInput};

fn main() -> Result<(), Box<dyn Error>> {
    println!("==================================================");
    println!("   ImpactSampler Studio - MIDI Port Sniffer v1.0  ");
    println!("==================================================");

    let midi_scan = MidiInput::new("ImpactSampler Scanner")?;
    let in_ports = midi_scan.ports();
    if in_ports.is_empty() {
        println!("\n[!] No se detectaron puertos MIDI de entrada en Windows.");
        println!("    Asegúrate de que el Nektar Impact LX49+ esté conectado y encendido.");
        return Ok(());
    }

    println!("\nPuertos MIDI detectados:");
    for (i, p) in in_ports.iter().enumerate() {
        let name = midi_scan.port_name(p).unwrap_or_else(|_| "Desconocido".to_string());
        println!("  [{}] {}", i, name);
    }

    // Conectamos a todos los puertos detectados para no perder ningún mensaje (ej. Puerto 0: Teclas/Pads y Puerto 1: Control)
    let mut _connections = Vec::new();
    println!("\n-> Conectando a los puertos detectados...");

    for (i, port) in in_ports.iter().enumerate() {
        let mut midi_in = MidiInput::new(&format!("ImpactSampler In-{}", i))?;
        midi_in.ignore(Ignore::None);
        let port_name = midi_scan.port_name(port)?;

        let p_name_clone = port_name.clone();
        let conn = midi_in.connect(
            port,
            &format!("impact-input-{}", i),
            move |stamp, message, _| {
                decode_midi_message(&p_name_clone, stamp, message);
            },
            (),
        );

        match conn {
            Ok(c) => {
                println!("  [OK] Conectado a [{}] '{}'", i, port_name);
                _connections.push(c);
            }
            Err(e) => {
                println!("  [WARN] No se pudo conectar a [{}]: {}", i, e);
            }
        }
    }

    println!("\n==================================================");
    println!(" [LISTO] Sniffer activo.");
    println!(" -> Toca teclas, pads, faders o perillas.");
    println!(" -> Presiona [ENTER] para finalizar el test.");
    println!("==================================================\n");

    let stdin = io::stdin();
    let mut line = String::new();
    let _ = stdin.lock().read_line(&mut line);

    println!("\nFinalizando sniffer...");
    Ok(())
}

fn decode_midi_message(port_name: &str, _timestamp: u64, msg: &[u8]) {
    if msg.is_empty() {
        return;
    }

    let status = msg[0];
    let channel = (status & 0x0F) + 1;
    let msg_type = status & 0xF0;

    let hex_repr: Vec<String> = msg.iter().map(|b| format!("{:02X}", b)).collect();
    let hex_str = hex_repr.join(" ");

    match msg_type {
        0x90 if msg.len() >= 3 => {
            let note = msg[1];
            let vel = msg[2];
            if vel == 0 {
                println!(
                    "[{}] Ch {} | Note Off (Vel 0): Nota {} ({})",
                    port_name, channel, note, get_note_name(note)
                );
            } else {
                let pad_desc = get_pad_desc(note);
                println!(
                    "[{}] Ch {} | NOTE ON: Nota {} ({}) | Vel: {}{}",
                    port_name, channel, note, get_note_name(note), vel, pad_desc
                );
            }
        }
        0x80 if msg.len() >= 3 => {
            let note = msg[1];
            let vel = msg[2];
            println!(
                "[{}] Ch {} | Note Off: Nota {} ({}) | Vel: {}",
                port_name, channel, note, get_note_name(note), vel
            );
        }
        0xB0 if msg.len() >= 3 => {
            let cc = msg[1];
            let val = msg[2];
            let cc_desc = get_cc_desc(cc);
            println!(
                "[{}] Ch {} | CC {}: Valor {}{}",
                port_name, channel, cc, val, cc_desc
            );
        }
        0xE0 if msg.len() >= 3 => {
            let lsb = msg[1] as u16;
            let msb = msg[2] as u16;
            let bend_val = (msb << 7) | lsb;
            let normalized = bend_val as i32 - 8192;
            println!(
                "[{}] Ch {} | PITCH BEND: {} (Centrado: {})",
                port_name, channel, bend_val, normalized
            );
        }
        _ => {
            println!(
                "[{}] Ch {} | RAW: [{}]",
                port_name, channel, hex_str
            );
        }
    }
}

fn get_pad_desc(note: u8) -> String {
    match note {
        36 => " [-> PAD 1]".to_string(),
        37 => " [-> PAD 2]".to_string(),
        38 => " [-> PAD 3]".to_string(),
        39 => " [-> PAD 4]".to_string(),
        40 => " [-> PAD 5]".to_string(),
        41 => " [-> PAD 6]".to_string(),
        42 => " [-> PAD 7]".to_string(),
        43 => " [-> PAD 8]".to_string(),
        _ => String::new(),
    }
}

fn get_cc_desc(cc: u8) -> String {
    match cc {
        // Faders (User Preset 1)
        73 => " [-> FADER 1 (Attack)]".to_string(),
        75 => " [-> FADER 2 (Decay)]".to_string(),
        72 => " [-> FADER 3 (Release)]".to_string(),
        91 => " [-> FADER 4 (FX 1)]".to_string(),
        92 => " [-> FADER 5 (FX 2)]".to_string(),
        93 => " [-> FADER 6 (FX 3)]".to_string(),
        94 => " [-> FADER 7 (FX 4)]".to_string(),
        95 => " [-> FADER 8 (FX 5)]".to_string(),
        7  => " [-> FADER 9 (MASTER VOLUME)]".to_string(),

        // Knobs
        74 => " [-> KNOB 1 (Cutoff)]".to_string(),
        71 => " [-> KNOB 2 (Resonance)]".to_string(),
        5  => " [-> KNOB 3 (Portamento Rate)]".to_string(),
        84 => " [-> KNOB 4 (Portamento Depth)]".to_string(),
        78 => " [-> KNOB 5 (Vibrato Delay)]".to_string(),
        76 => " [-> KNOB 6 (Vibrato Rate)]".to_string(),
        77 => " [-> KNOB 7 (Vibrato Depth)]".to_string(),
        10 => " [-> KNOB 8 (Global Pan)]".to_string(),

        // Wheels
        1  => " [-> MODULATION WHEEL]".to_string(),

        // Botones bajo faders
        0  => " [-> BOTÓN B1]".to_string(),
        2  => " [-> BOTÓN B2]".to_string(),
        3  => " [-> BOTÓN B3]".to_string(),
        4  => " [-> BOTÓN B4]".to_string(),
        6  => " [-> BOTÓN B5]".to_string(),
        8  => " [-> BOTÓN B6]".to_string(),
        9  => " [-> BOTÓN B7]".to_string(),
        11 => " [-> BOTÓN B8]".to_string(),
        65 => " [-> BOTÓN B9]".to_string(),

        _ => String::new(),
    }
}

fn get_note_name(note: u8) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (note as i32 / 12) - 1;
    let name = names[(note % 12) as usize];
    format!("{}{}", name, octave)
}
