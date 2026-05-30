#[cfg(target_os = "windows")]
mod wasapi_loopback {
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};

    type ChunkCallback = Box<dyn Fn(&[f32], u32) + Send>;

    pub struct LoopbackCapture {
        running: Arc<AtomicBool>,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        sample_rate: Arc<std::sync::atomic::AtomicU32>,
        channels: Arc<std::sync::atomic::AtomicU32>,
    }

    impl LoopbackCapture {
        pub fn new() -> Self {
            Self {
                running: Arc::new(AtomicBool::new(false)),
                audio_buffer: Arc::new(Mutex::new(Vec::new())),
                sample_rate: Arc::new(std::sync::atomic::AtomicU32::new(48000)),
                channels: Arc::new(std::sync::atomic::AtomicU32::new(2)),
            }
        }

        pub fn start(&self, device_name: &str, chunk_callback: ChunkCallback) -> Result<(), String> {
            if self.running.load(Ordering::SeqCst) {
                return Err("Loopback capture already running".to_string());
            }

            self.running.store(true, Ordering::SeqCst);
            let running = self.running.clone();
            let buffer = self.audio_buffer.clone();
            let sample_rate_arc = self.sample_rate.clone();
            let channels_arc = self.channels.clone();
            let device_name = device_name.to_string();

            std::thread::spawn(move || {
                if let Err(e) = run_loopback_capture(&device_name, &running, &buffer, &sample_rate_arc, &channels_arc, &chunk_callback) {
                    log::error!("WASAPI loopback capture error: {}", e);
                }
                running.store(false, Ordering::SeqCst);
            });

            Ok(())
        }

        pub fn stop(&self) -> Result<(), String> {
            if !self.running.load(Ordering::SeqCst) {
                return Ok(());
            }
            self.running.store(false, Ordering::SeqCst);
            Ok(())
        }

        pub fn is_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }

        pub fn get_buffered_audio(&self) -> Vec<f32> {
            let mut buf = self.audio_buffer.lock().unwrap();
            let data = buf.clone();
            buf.clear();
            data
        }

        pub fn sample_rate(&self) -> u32 {
            self.sample_rate.load(Ordering::SeqCst)
        }

        pub fn channels(&self) -> u32 {
            self.channels.load(Ordering::SeqCst)
        }
    }

    fn run_loopback_capture(
        device_name: &str,
        running: &AtomicBool,
        buffer: &Mutex<Vec<f32>>,
        sample_rate_arc: &std::sync::atomic::AtomicU32,
        channels_arc: &std::sync::atomic::AtomicU32,
        callback: &ChunkCallback,
    ) -> Result<(), String> {
        use windows::Win32::Media::Audio::{
            eRender, eConsole, AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioClient, IAudioCaptureClient,
        };
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, CoCreateInstance, CLSCTX_ALL, COINIT_MULTITHREADED,
        };

        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()
                .map_err(|e| format!("COM init failed: {}", e))?;

            let enumerator: windows::Win32::Media::Audio::IMMDeviceEnumerator =
                CoCreateInstance(
                    &windows::Win32::Media::Audio::MMDeviceEnumerator,
                    None,
                    CLSCTX_ALL,
                )
                .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

            let device = if device_name.is_empty() {
                enumerator.GetDefaultAudioEndpoint(eRender, eConsole)
                    .map_err(|e| format!("Failed to get default render device: {}", e))?
            } else {
                let collection = enumerator
                    .EnumAudioEndpoints(eRender, windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE)
                    .map_err(|e| format!("Failed to enumerate render devices: {}", e))?;

                let count = collection
                    .GetCount()
                    .map_err(|e| format!("Failed to get device count: {}", e))?;

                let mut found = None;
                for i in 0..count {
                    if let Ok(dev) = collection.Item(i) {
                        let id = dev.GetId()
                            .map_err(|e| format!("Failed to get device id: {}", e))?;
                        let id_str = id.to_string().unwrap_or_default();
                        if id_str.contains(device_name) {
                            found = Some(dev);
                            break;
                        }
                    }
                }
                found.ok_or_else(|| format!("Device '{}' not found", device_name))?
            };

            let audio_client: IAudioClient = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Failed to activate audio client: {}", e))?;

            let mix_format = audio_client.GetMixFormat()
                .map_err(|e| format!("Failed to get mix format: {}", e))?;

            let wave_format = &*mix_format;
            let n_channels = wave_format.nChannels as u32;
            let n_samples_per_sec = wave_format.nSamplesPerSec;

            sample_rate_arc.store(n_samples_per_sec, Ordering::SeqCst);
            channels_arc.store(n_channels, Ordering::SeqCst);

            let buffer_duration = 10_000_000;
            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                buffer_duration,
                0,
                mix_format as *const _,
                None,
            ).map_err(|e| format!("Failed to initialize loopback client: {}", e))?;

            let capture_client: IAudioCaptureClient = audio_client.GetService()
                .map_err(|e| format!("Failed to get capture client: {}", e))?;

            audio_client.Start()
                .map_err(|e| format!("Failed to start audio client: {}", e))?;

            let chunk_frames = n_samples_per_sec / 10;
            let mut chunk_buffer: Vec<f32> = Vec::with_capacity((chunk_frames * n_channels) as usize);

            while running.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));

                loop {
                    let packet_size = match capture_client.GetNextPacketSize() {
                        Ok(s) => s,
                        Err(_) => break,
                    };
                    if packet_size == 0 {
                        break;
                    }

                    let mut data_ptr: *mut u8 = std::ptr::null_mut();
                    let mut num_frames: u32 = 0;
                    let mut flags: u32 = 0;

                    let hr = capture_client.GetBuffer(
                        &mut data_ptr,
                        &mut num_frames,
                        &mut flags,
                        None,
                        None,
                    );

                    if hr.is_err() || num_frames == 0 || data_ptr.is_null() {
                        break;
                    }

                    let sample_count = (num_frames * n_channels) as usize;
                    let samples = std::slice::from_raw_parts(data_ptr as *const f32, sample_count);

                    chunk_buffer.extend_from_slice(samples);

                    {
                        let mut buf = buffer.lock().unwrap();
                        buf.extend_from_slice(samples);
                    }

                    if chunk_buffer.len() >= (chunk_frames * n_channels) as usize {
                        callback(&chunk_buffer, n_channels);
                        chunk_buffer.clear();
                    }

                    let _ = capture_client.ReleaseBuffer(num_frames);
                }
            }

            let _ = audio_client.Stop();
            CoUninitialize();
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub use wasapi_loopback::LoopbackCapture;

#[cfg(not(target_os = "windows"))]
pub struct LoopbackCapture;

#[cfg(not(target_os = "windows"))]
impl LoopbackCapture {
    pub fn new() -> Self { Self }
    pub fn start(&self, _device_name: &str, _callback: Box<dyn Fn(&[f32], u32) + Send>) -> Result<(), String> {
        Err("WASAPI loopback is only supported on Windows".to_string())
    }
    pub fn stop(&self) -> Result<(), String> { Ok(()) }
    pub fn is_running(&self) -> bool { false }
    pub fn get_buffered_audio(&self) -> Vec<f32> { Vec::new() }
    pub fn sample_rate(&self) -> u32 { 48000 }
    pub fn channels(&self) -> u32 { 2 }
}
