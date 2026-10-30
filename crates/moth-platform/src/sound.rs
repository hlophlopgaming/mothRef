// SPDX-License-Identifier: GPL-3.0-or-later
//! Short sound cues (drawing sessions) without an audio library: the samples are
//! synthesized once as WAV bytes and handed to what each OS already has.
//!
//! Playing never blocks the caller: requests go to one worker thread through a small
//! bounded channel, and a cue that cannot be queued is dropped rather than delayed.
use std::sync::OnceLock;

/// The cues the app plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// A soft two-note chime: the next image is up.
    Chime,
    /// A quiet click: one of the last seconds.
    Tick,
}
/// Queues a cue; silently does nothing where no player is available.
pub fn play(sound: Sound) {
    static WORKER: OnceLock<Option<crossbeam_channel::Sender<Sound>>> = OnceLock::new();
    let sender = WORKER.get_or_init(|| {
        let (sender, receiver) = crossbeam_channel::bounded::<Sound>(4);
        std::thread::Builder::new()
            .name("sound".into())
            .spawn(move || {
                let mut player = Player::default();
                while let Ok(sound) = receiver.recv() {
                    // A failing backend must not take the thread, and with it every
                    // later cue, down.
                    let played = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        player.play(sound)
                    }));
                    if !matches!(played, Ok(true)) {
                        tracing::debug!(?sound, "sound cue not played");
                    }
                }
            })
            .ok()
            .map(|_| sender)
    });
    if let Some(sender) = sender {
        let _ = sender.try_send(sound);
    }
}
/// 16-bit mono PCM WAV of a cue.
pub fn wav(sound: Sound) -> Vec<u8> {
    const RATE: u32 = 22_050;
    let (notes, length, volume): (&[(f32, f32)], f32, f32) = match sound {
        // E6 then A6, overlapping, like a small bell.
        Sound::Chime => (&[(1318.5, 0.), (1760., 0.09)], 0.45, 0.28),
        Sound::Tick => (&[(2093., 0.)], 0.035, 0.12),
    };
    let count = (RATE as f32 * length) as usize;
    let samples: Vec<i16> = (0..count)
        .map(|index| {
            let t = index as f32 / RATE as f32;
            let value: f32 = notes
                .iter()
                .filter(|(_, start)| t >= *start)
                .map(|(frequency, start)| {
                    let local = t - start;
                    let attack = (local / 0.004).min(1.);
                    let decay = (-local * if sound == Sound::Tick { 90. } else { 9. }).exp();
                    (std::f32::consts::TAU * frequency * local).sin() * attack * decay
                })
                .sum();
            (value * volume * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16
        })
        .collect();
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Plays through the first command-line player that works: PipeWire, PulseAudio, ALSA.
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
#[derive(Default)]
struct Player {
    files: Option<[std::path::PathBuf; 2]>,
    command: Option<&'static str>,
    broken: bool,
}
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
impl Player {
    fn play(&mut self, sound: Sound) -> bool {
        if self.broken {
            return false;
        }
        if self.files.is_none() {
            // The app's own runtime or temp folder; the files are tiny and rewritten
            // each launch.
            let directory = std::env::var_os("XDG_RUNTIME_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(format!("mothref-sounds-{}", std::process::id()));
            let files = [Sound::Chime, Sound::Tick].map(|sound| {
                directory.join(if sound == Sound::Chime {
                    "chime.wav"
                } else {
                    "tick.wav"
                })
            });
            let written = std::fs::create_dir_all(&directory).is_ok()
                && [Sound::Chime, Sound::Tick]
                    .iter()
                    .zip(&files)
                    .all(|(sound, path)| std::fs::write(path, wav(*sound)).is_ok());
            if !written {
                self.broken = true;
                return false;
            }
            self.files = Some(files);
        }
        let Some(files) = &self.files else {
            return false;
        };
        let file = &files[if sound == Sound::Chime { 0 } else { 1 }];
        let candidates: &[&'static str] = match self.command {
            Some(command) => &[command][..],
            None => &["pw-play", "paplay", "aplay"],
        };
        for command in candidates.iter().copied() {
            let mut process = std::process::Command::new(command);
            if command == "aplay" {
                process.arg("-q");
            }
            let status = process
                .arg(file)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if status.is_ok_and(|status| status.success()) {
                self.command = Some(command);
                return true;
            }
        }
        if self.command.is_none() {
            self.broken = true;
        }
        false
    }
}

#[cfg(windows)]
#[derive(Default)]
struct Player {
    clips: Option<[Vec<u8>; 2]>,
}
#[cfg(windows)]
impl Player {
    fn play(&mut self, sound: Sound) -> bool {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
        let clips = self
            .clips
            .get_or_insert_with(|| [wav(Sound::Chime), wav(Sound::Tick)]);
        let clip = &clips[if sound == Sound::Chime { 0 } else { 1 }];
        // SAFETY: with SND_MEMORY the first argument points at a complete WAV image;
        // SND_SYNC returns only after playback, while `clip` is still borrowed here.
        unsafe {
            PlaySoundW(
                clip.as_ptr().cast(),
                std::ptr::null_mut(),
                SND_MEMORY | SND_SYNC | SND_NODEFAULT,
            ) != 0
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct Player {
    current: Option<objc2::rc::Retained<objc2_app_kit::NSSound>>,
}
#[cfg(target_os = "macos")]
impl Player {
    fn play(&mut self, sound: Sound) -> bool {
        use objc2::AllocAnyThread;
        let data = objc2_foundation::NSData::with_bytes(&wav(sound));
        let Some(player) =
            objc2_app_kit::NSSound::initWithData(objc2_app_kit::NSSound::alloc(), &data)
        else {
            return false;
        };
        let started = player.play();
        // NSSound stops when released; keep it until the next cue replaces it.
        self.current = Some(player);
        started
    }
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
#[derive(Default)]
struct Player;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
impl Player {
    fn play(&mut self, _: Sound) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_short_valid_wavs() {
        for sound in [Sound::Chime, Sound::Tick] {
            let bytes = wav(sound);
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(
                u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
                bytes.len() - 8
            );
            assert!(bytes.len() < 32 * 1024);
        }
    }
}
