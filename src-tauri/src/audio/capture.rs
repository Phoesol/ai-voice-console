use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex, LazyLock, mpsc};

struct RecordingSession {
    sample_rate: u32,
    buffers: Arc<Mutex<Vec<f32>>>,
    stop_flag: Arc<Mutex<bool>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

static RECORDING: LazyLock<Mutex<Option<RecordingSession>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn start_capture(device_name: Option<&str>, _monitor_device_name: Option<&str>) -> Result<u32, String> {
    stop_capture();

    let host = cpal::default_host();
    let device = if let Some(name) = device_name {
        host.input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
            .find(|d| d.name().map(|n| n.contains(name)).unwrap_or(false))
            .ok_or_else(|| format!("Input device '{}' not found", name))?
    } else {
        host.default_input_device()
            .ok_or("No default input device available")?
    };

    let config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {}", e))?;

    let device_sr = config.sample_rate().0;
    let channels = config.channels();
    let device_name_str = device.name().unwrap_or_default();
    log::info!("[录音] Device: {}, SR: {}Hz, Channels: {}", device_name_str, device_sr, channels);

    let buffers: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let stop_flag: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let buffers_clone = buffers.clone();
    let stop_flag_clone = stop_flag.clone();

    // 麦克风侧音已移除 — 如需要可在 TTS 播放时通过 monitor-device 试听
    // monitor_device_name 参数保留兼容性，但不再创建侧音流

    let (ready_tx, ready_rx) = mpsc::channel();

    let handle = std::thread::Builder::new()
        .name("audio-capture".into())
        .spawn(move || {
            let err_fn = |err: cpal::StreamError| {
                log::error!("[录音] Stream error: {}", err);
            };

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    let bufs = buffers_clone.clone();
                    let stop = stop_flag_clone.clone();
                    let ch = channels;
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if *stop.lock().unwrap() { return; }
                            let mono: Vec<f32> = if ch > 1 {
                                data.chunks_exact(ch as usize)
                                    .map(|frame| frame.iter().sum::<f32>() / ch as f32)
                                    .collect()
                            } else {
                                data.to_vec()
                            };
                            bufs.lock().unwrap().extend_from_slice(&mono);
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let bufs = buffers_clone.clone();
                    let stop = stop_flag_clone.clone();
                    let ch = channels;
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            if *stop.lock().unwrap() { return; }
                            let mono: Vec<f32> = if ch > 1 {
                                data.chunks_exact(ch as usize)
                                    .map(|frame| {
                                        frame.iter().map(|&s| s as f32 / 32768.0).sum::<f32>()
                                            / ch as f32
                                    })
                                    .collect()
                            } else {
                                data.iter().map(|&s| s as f32 / 32768.0).collect()
                            };
                            bufs.lock().unwrap().extend_from_slice(&mono);
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    let bufs = buffers_clone.clone();
                    let stop = stop_flag_clone.clone();
                    let ch = channels;
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[u16], _: &cpal::InputCallbackInfo| {
                            if *stop.lock().unwrap() { return; }
                            let mono: Vec<f32> = if ch > 1 {
                                data.chunks_exact(ch as usize)
                                    .map(|frame| {
                                        frame.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum::<f32>()
                                            / ch as f32
                                    })
                                    .collect()
                            } else {
                                data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect()
                            };
                            bufs.lock().unwrap().extend_from_slice(&mono);
                        },
                        err_fn,
                        None,
                    )
                }
                fmt => {
                    let _ = ready_tx.send(Err(format!("Unsupported sample format: {:?}", fmt)));
                    return;
                }
            };

            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Failed to build input stream: {}", e)));
                    return;
                }
            };

            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(format!("Failed to start recording: {}", e)));
                return;
            }

            let _ = ready_tx.send(Ok(()));

            while !*stop_flag_clone.lock().unwrap() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }

            drop(stream);
            log::info!("[录音] Stream dropped, recording stopped");
        })
        .map_err(|e| format!("Failed to spawn capture thread: {}", e))?;

    ready_rx.recv().map_err(|e| format!("Capture thread panicked: {}", e))??;

    let mut rec = RECORDING.lock().unwrap();
    *rec = Some(RecordingSession {
        sample_rate: device_sr,
        buffers,
        stop_flag,
        thread_handle: Some(handle),
    });

    log::info!("[录音] Recording started @ {}Hz", device_sr);
    Ok(device_sr)
}

pub fn stop_capture() -> Option<(Vec<f32>, u32)> {
    let mut rec = RECORDING.lock().unwrap();
    let session = rec.take()?;

    *session.stop_flag.lock().unwrap() = true;

    if let Some(handle) = session.thread_handle {
        let _ = handle.join();
    }

    let samples = {
        let mut buf = session.buffers.lock().unwrap();
        std::mem::take(&mut *buf)
    };

    let sr = session.sample_rate;

    if samples.is_empty() {
        log::info!("[录音] No audio data captured");
        return None;
    }

    let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let duration = samples.len() as f64 / sr as f64;
    log::info!(
        "[录音] Captured: {} samples, {:.2}s, peak={:.4}, rms={:.4} @ {}Hz",
        samples.len(), duration, peak, rms, sr
    );

    Some((samples, sr))
}

