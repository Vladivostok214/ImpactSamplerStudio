use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(target_os = "windows")]
unsafe extern "C" {
    #[link_name = "?ASIOControlPanel@@YAJXZ"]
    fn ASIOControlPanel() -> i32;
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("ASIO test is only supported on Windows.");
}

#[cfg(target_os = "windows")]
fn main() {
    println!("=== TEST DE AUDIO ASIO DIRECTO CON ZOOM F3 ===");
    let asio = asio_sys::Asio::new();
    let driver_name = "ZOOM F3 ASIO Driver";
    println!("Cargando controlador '{}'...", driver_name);
    
    let driver = match asio.load_driver(driver_name) {
        Ok(d) => d,
        Err(e) => {
            println!("No se pudo cargar el driver: {:?}", e);
            return;
        }
    };

    println!("Probando invocación de ASIOControlPanel()...");
    #[cfg(target_os = "windows")]
    unsafe {
        let res = ASIOControlPanel();
        println!("ASIOControlPanel devolvió código: {}", res);
    }

    let sample_rate = driver.sample_rate().unwrap_or(44100.0);
    println!("Driver cargado exitosamente. Sample rate: {} Hz", sample_rate);

    let data_type = driver.output_data_type().expect("Error obteniendo tipo de datos");
    println!("Tipo de muestra del driver: {:?}", data_type);

    let streams = driver.prepare_output_stream(None, 2, None).expect("Error preparando streams");
    let output_stream = streams.output.expect("No se crearon buffers de salida");
    let buffer_size = output_stream.buffer_size;
    println!("Buffers de audio creados. Buffer size: {} frames", buffer_size);

    // Guardar las direcciones de memoria como usize para transferirlas de forma thread-safe
    let left_b0 = output_stream.buffer_infos[0].buffers[0] as usize;
    let left_b1 = output_stream.buffer_infos[0].buffers[1] as usize;
    let right_b0 = output_stream.buffer_infos[1].buffers[0] as usize;
    let right_b1 = output_stream.buffer_infos[1].buffers[1] as usize;

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();

    let mut phase = 0.0f32;
    let phase_inc = 2.0 * std::f32::consts::PI * 440.0 / (sample_rate as f32);

    let _cb_id = driver.add_callback(move |info| {
        if !is_running_clone.load(Ordering::Relaxed) {
            return;
        }
        let buf_idx = info.buffer_index;
        let left_ptr = if buf_idx == 0 { left_b0 as *mut i32 } else { left_b1 as *mut i32 };
        let right_ptr = if buf_idx == 0 { right_b0 as *mut i32 } else { right_b1 as *mut i32 };

        let frames = buffer_size as usize;
        for i in 0..frames {
            let sample = (phase.sin() * 0.25 * 2147483647.0) as i32;
            phase += phase_inc;
            if phase > 2.0 * std::f32::consts::PI {
                phase -= 2.0 * std::f32::consts::PI;
            }
            unsafe {
                *left_ptr.add(i) = sample;
                *right_ptr.add(i) = sample;
            }
        }
    });

    println!("Iniciando reproducción de tono de prueba (440 Hz) durante 1.5 segundos...");
    driver.start().expect("Error al iniciar driver");
    std::thread::sleep(std::time::Duration::from_millis(1500));

    println!("Deteniendo driver...");
    is_running.store(false, Ordering::Relaxed);
    let _ = driver.stop();
    let _ = driver.dispose_buffers();
    println!("¡PRUEBA NATIVA DE AUDIO ASIO FINALIZADA CON ÉXITO!");
}
