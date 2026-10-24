// SPDX-License-Identifier: GPL-3.0-or-later
//! Video-memory detection and the texture limit derived from it.
//!
//! wgpu does not report how much memory an adapter has, so each OS is asked directly:
//! sysfs and `nvidia-smi` on Linux, the display-adapter registry class on Windows.
//! macOS and integrated adapters share system RAM. Detection spawns processes and
//! reads files, so it always runs on a worker thread.

/// How an adapter's memory relates to system RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterKind {
    /// Dedicated video memory.
    Discrete,
    /// Shares system RAM (integrated GPUs, Apple silicon).
    Integrated,
    /// Renders on the CPU; textures live in system RAM.
    Software,
    /// Unknown adapter type.
    Other,
}

/// Result of detection, shown to the user next to the limit it produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoMemory {
    /// Adapter name as reported by the driver.
    pub name: String,
    /// Adapter memory model.
    pub kind: AdapterKind,
    /// Dedicated video memory, when the OS reported it.
    pub dedicated: Option<u64>,
    /// Physical RAM, the budget for shared-memory adapters.
    pub system: Option<u64>,
    /// Texture limit the app uses unless the user sets one.
    pub limit: u64,
}

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
/// Never below what a single full-detail board view needs.
const FLOOR: u64 = 256 * MIB;

/// Texture limit for an adapter: most of dedicated video memory, leaving room for the
/// desktop compositor and other applications; half of RAM for shared-memory adapters.
pub fn limit_for(kind: AdapterKind, dedicated: Option<u64>, system: Option<u64>) -> u64 {
    let ram = system.unwrap_or(8 * GIB);
    let limit = match (kind, dedicated) {
        // The compositor and other GPU apps keep some VRAM; never plan to use all of it.
        (AdapterKind::Discrete, Some(vram)) => (vram / 10 * 8).min(vram.saturating_sub(512 * MIB)),
        (AdapterKind::Discrete, None) => (4 * GIB).min(ram / 2),
        (AdapterKind::Software, _) => ram / 4,
        (AdapterKind::Integrated | AdapterKind::Other, _) => ram / 2,
    };
    limit.max(FLOOR)
}

/// Detects the adapter's memory. Blocking: call it on a worker thread.
pub fn detect(name: &str, kind: AdapterKind) -> VideoMemory {
    let system = {
        let mut system = sysinfo::System::new();
        system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
        Some(system.total_memory()).filter(|bytes| *bytes > 0)
    };
    let dedicated = if kind == AdapterKind::Discrete {
        pick(name, &adapters())
    } else {
        None
    };
    VideoMemory {
        name: name.to_owned(),
        kind,
        dedicated,
        system,
        limit: limit_for(kind, dedicated, system),
    }
}

/// Runs [`detect`] on its own thread and reports the result once.
pub fn spawn(
    name: String,
    kind: AdapterKind,
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
) -> crossbeam_channel::Receiver<VideoMemory> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let spawned = std::thread::Builder::new()
        .name("gpu-memory".into())
        .spawn(move || {
            if tx.send(detect(&name, kind)).is_ok() {
                wake();
            }
        });
    if let Err(error) = spawned {
        tracing::debug!(%error, "video memory detection unavailable");
    }
    rx
}

/// Picks the reported adapter matching `name`, else the only one, else the largest.
fn pick(name: &str, adapters: &[(String, u64)]) -> Option<u64> {
    let wanted = name.to_lowercase();
    adapters
        .iter()
        .find(|(reported, _)| {
            let reported = reported.to_lowercase();
            !reported.is_empty() && (wanted.contains(&reported) || reported.contains(&wanted))
        })
        .or_else(|| adapters.iter().max_by_key(|(_, bytes)| *bytes))
        .map(|(_, bytes)| *bytes)
        .filter(|bytes| *bytes >= 64 * MIB)
}

/// Runs a short query command, giving up after a few seconds.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn query(program: &str, args: &[&str]) -> Option<String> {
    let mut command = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Registry and driver probes are console executables even when MothRef is a GUI.
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let started = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    }
    let mut output = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut output).ok()?;
    Some(output)
}

#[cfg(target_os = "linux")]
fn adapters() -> Vec<(String, u64)> {
    let mut found = Vec::new();
    // amdgpu and Intel discrete (xe) report VRAM through sysfs.
    if let Ok(cards) = std::fs::read_dir("/sys/class/drm") {
        for card in cards.flatten() {
            let device = card.path().join("device");
            for file in ["mem_info_vram_total", "lmem_total_bytes"] {
                if let Some(bytes) = std::fs::read_to_string(device.join(file))
                    .ok()
                    .and_then(|text| text.trim().parse::<u64>().ok())
                {
                    let name = std::fs::read_to_string(device.join("product_name"))
                        .unwrap_or_default()
                        .trim()
                        .to_owned();
                    found.push((name, bytes));
                }
            }
        }
    }
    found.extend(nvidia());
    found
}
#[cfg(target_os = "windows")]
fn adapters() -> Vec<(String, u64)> {
    let mut found = nvidia();
    if let Some(text) = query(
        "reg",
        &[
            "query",
            r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}",
            "/s",
        ],
    ) {
        found.extend(parse_registry(&text));
    }
    found
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn adapters() -> Vec<(String, u64)> {
    Vec::new()
}

/// `nvidia-smi` exists wherever the proprietary NVIDIA driver is installed.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn nvidia() -> Vec<(String, u64)> {
    query(
        "nvidia-smi",
        &[
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ],
    )
    .map(|text| parse_nvidia(&text))
    .unwrap_or_default()
}
/// Lines like `NVIDIA GeForce RTX 3060, 12288` (MiB).
#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
fn parse_nvidia(text: &str) -> Vec<(String, u64)> {
    text.lines()
        .filter_map(|line| {
            let (name, mib) = line.rsplit_once(',')?;
            let mib: u64 = mib.trim().parse().ok()?;
            Some((name.trim().to_owned(), mib * MIB))
        })
        .collect()
}
/// Each adapter subkey of the display class lists `DriverDesc` and
/// `HardwareInformation.qwMemorySize` (a 64-bit size, unlike the 32-bit legacy value).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_registry(text: &str) -> Vec<(String, u64)> {
    let mut found = Vec::new();
    let mut name = String::new();
    let mut bytes = None;
    let mut flush = |name: &mut String, bytes: &mut Option<u64>| {
        if let Some(size) = bytes.take() {
            found.push((std::mem::take(name), size));
        }
        name.clear();
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("HKEY_") {
            flush(&mut name, &mut bytes);
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(key), Some(kind)) = (parts.next(), parts.next()) else {
            continue;
        };
        let value = parts.collect::<Vec<_>>().join(" ");
        match (key, kind) {
            ("DriverDesc", "REG_SZ") => name = value,
            ("HardwareInformation.qwMemorySize", "REG_QWORD") => {
                bytes = u64::from_str_radix(value.trim_start_matches("0x"), 16).ok();
            }
            _ => {}
        }
    }
    flush(&mut name, &mut bytes);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn hidden_probes_preserve_captured_output() {
        assert_eq!(
            query("cmd", &["/C", "echo mothref-probe"])
                .as_deref()
                .map(str::trim),
            Some("mothref-probe")
        );
    }
    #[test]
    fn limits_leave_room_for_the_desktop_and_follow_the_memory_model() {
        let ram = Some(32 * GIB);
        assert_eq!(
            limit_for(AdapterKind::Discrete, Some(12 * GIB), ram),
            12 * GIB / 10 * 8
        );
        assert_eq!(
            limit_for(AdapterKind::Discrete, Some(2 * GIB), ram),
            2 * GIB - 512 * MIB
        );
        assert_eq!(limit_for(AdapterKind::Discrete, None, ram), 4 * GIB);
        assert_eq!(limit_for(AdapterKind::Integrated, None, ram), 16 * GIB);
        assert_eq!(limit_for(AdapterKind::Software, None, ram), 8 * GIB);
        assert_eq!(limit_for(AdapterKind::Discrete, Some(MIB), ram), FLOOR);
    }

    #[test]
    fn reported_adapters_are_parsed_and_matched_by_name() {
        let nvidia = parse_nvidia("NVIDIA GeForce RTX 3060, 12288\nbroken line\n");
        assert_eq!(nvidia, vec![("NVIDIA GeForce RTX 3060".into(), 12 * GIB)]);
        let registry = parse_registry(
            "HKEY_LOCAL_MACHINE\\...\\0000\n    DriverDesc    REG_SZ    Intel(R) UHD Graphics\n\
             HKEY_LOCAL_MACHINE\\...\\0001\n    DriverDesc    REG_SZ    AMD Radeon RX 6800\n    \
             HardwareInformation.qwMemorySize    REG_QWORD    0x400000000\n",
        );
        assert_eq!(registry, vec![("AMD Radeon RX 6800".into(), 16 * GIB)]);
        let both = [nvidia[0].clone(), registry[0].clone()];
        assert_eq!(
            pick("AMD Radeon RX 6800 (RADV NAVI21)", &both),
            Some(16 * GIB)
        );
        assert_eq!(pick("Unknown adapter", &both), Some(16 * GIB));
        assert_eq!(pick("anything", &[]), None);
    }
}
