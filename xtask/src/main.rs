// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! MothRef measurement tools.
mod load;
mod reliability;
use std::time::{Duration, Instant};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("load") => load::run(
            std::path::Path::new(args.get(1).ok_or_else(|| {
                anyhow::anyhow!("usage: cargo xtask load <board-or-image-directory> [save-as.moth]")
            })?),
            args.get(2).map(std::path::Path::new),
        )?,
        Some("load-writer") => {
            reliability::load_writer(args.get(1).ok_or_else(|| anyhow::anyhow!("missing path"))?)?
        }
        Some("load-killtest") => {
            reliability::load_killtest(args.get(1).map(|v| v.parse()).transpose()?.unwrap_or(50))?
        }
        Some("killtest") => {
            reliability::killtest(args.get(1).map(|v| v.parse()).transpose()?.unwrap_or(1000))?
        }
        Some("kill-writer") => {
            reliability::writer(args.get(1).ok_or_else(|| anyhow::anyhow!("missing path"))?)?
        }
        Some("fixture") => reliability::fixture()?,
        Some("bench") => {
            let mut harness = pollster::block_on(moth_render::headless::Harness::new(
                args.iter().any(|a| a == "--software"),
            ))
            .map_err(anyhow::Error::msg)?;
            let result = harness
                .measure(if args.iter().any(|a| a == "--quick") {
                    100
                } else {
                    1000
                })
                .map_err(anyhow::Error::msg)?;
            println!(
                "10,000 distinct images | {} | {} frames | p99 {:.3} ms | textures {:.2} MiB",
                result.adapter,
                result.frames,
                result.p99_ms,
                result.texture_bytes as f64 / 1048576.
            );
            if args.iter().any(|a| a == "--enforce") {
                anyhow::ensure!(result.p99_ms <= 12., "M0 p99 exceeds 12 ms");
            }
        }
        Some("paste") => {
            let storage = moth_platform::DesktopStorage::default();
            let mut pixels = vec![0u8; 3840 * 2160 * 4];
            let mut seed = 42u32;
            for pixel in pixels.as_chunks_mut::<4>().0 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                pixel.copy_from_slice(&[seed as u8, (seed >> 8) as u8, (seed >> 16) as u8, 255]);
            }
            let mut harness = pollster::block_on(moth_render::headless::Harness::new(false))
                .map_err(anyhow::Error::msg)?;
            harness.frame(0).map_err(anyhow::Error::msg)?;
            let start = Instant::now();
            let (_, image) = storage
                .stage_preview(3840, 2160, &pixels)
                .map_err(anyhow::Error::msg)?;
            let staged = start.elapsed();
            harness.show(&image).map_err(anyhow::Error::msg)?;
            let elapsed = start.elapsed();
            println!(
                "4K paste pipeline after clipboard transfer: {:.3} ms (durable staging {:.3} ms)",
                elapsed.as_secs_f64() * 1000.,
                staged.as_secs_f64() * 1000.
            );
            storage.discard()?;
            if args.iter().any(|arg| arg == "--enforce") {
                anyhow::ensure!(
                    elapsed.as_secs_f64() * 1000. <= 250.,
                    "4K paste pipeline exceeds 250 ms (accepted ~200 ms plus timing margin)"
                );
            }
        }
        Some("soak") => {
            let seconds = args
                .get(1)
                .map(|s| s.parse())
                .transpose()?
                .unwrap_or(1800u64);
            // Short smoke runs need denser RSS samples for the plateau check to apply.
            let interval = args
                .get(2)
                .map(|s| s.parse())
                .transpose()?
                .unwrap_or(30u64)
                .max(1);
            let mut harness = pollster::block_on(moth_render::headless::Harness::new(false))
                .map_err(anyhow::Error::msg)?;
            let persistence = reliability::Soak::new()?;
            let start = Instant::now();
            let mut frame = 0;
            let mut samples = Vec::new();
            let mut next = 0;
            while start.elapsed() < Duration::from_secs(seconds) {
                harness.frame(frame).map_err(anyhow::Error::msg)?;
                persistence.step(frame)?;
                frame += 1;
                if start.elapsed().as_secs() >= next {
                    if let Ok(stat) = std::fs::read_to_string("/proc/self/status")
                        && let Some(line) = stat.lines().find(|s| s.starts_with("VmRSS:"))
                    {
                        let rss = line
                            .split_whitespace()
                            .nth(1)
                            .unwrap_or("0")
                            .parse::<u64>()?;
                        println!("{} s: {} KiB RSS", start.elapsed().as_secs(), rss);
                        samples.push(rss);
                    }
                    next += interval;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if samples.len() > 3 {
                let tail = &samples[samples.len() / 2..];
                let low = *tail.iter().min().unwrap_or(&0);
                let high = *tail.iter().max().unwrap_or(&0);
                anyhow::ensure!(high - low <= 16384, "RSS failed to plateau within 16 MiB");
            }
            persistence.finish()?;
            println!(
                "soak complete: {frame} frames in {} s",
                start.elapsed().as_secs()
            );
        }
        Some("pur") => {
            let path = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("usage: cargo xtask pur <board.pur>"))?;
            reliability::roundtrip_pur(std::path::Path::new(path))?;
        }
        Some("corpus") => {
            let mut count = 0;
            let mut directories = vec![std::path::PathBuf::from("corpus")];
            while let Some(directory) = directories.pop() {
                for entry in std::fs::read_dir(directory)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        directories.push(path);
                        continue;
                    }
                    match path.extension().and_then(|s| s.to_str()) {
                        Some("png" | "jpg" | "jpeg" | "rgba" | "gif" | "bmp" | "webp") => {
                            let bytes = std::fs::read(&path)?;
                            let decoded = moth_io::decode(&bytes, 256)?;
                            anyhow::ensure!(
                                !decoded.mips.is_empty(),
                                "empty LOD pyramid: {}",
                                path.display()
                            );
                            count += 1;
                        }
                        Some("moth") => {
                            reliability::roundtrip(&path)?;
                            count += 1;
                        }
                        Some("bee") => {
                            let file = std::fs::File::open(&path)?;
                            let summary = moth_import::bee::read(file, &mut |_| Ok(()))
                                .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
                            anyhow::ensure!(
                                summary.images > 0,
                                "no images imported from {}",
                                path.display()
                            );
                            count += 1;
                        }
                        Some("pur") => {
                            reliability::roundtrip_pur(&path)?;
                            count += 1;
                        }
                        _ => {}
                    }
                }
            }
            anyhow::ensure!(count > 0, "image corpus is empty");
            println!("{count} image and board fixtures validated");
        }
        _ => anyhow::bail!(
            "usage: cargo xtask load <board-or-image-directory> [save-as.moth] | load-killtest [count] | bench [--quick] [--enforce] [--software] | paste [--enforce] | soak [seconds] [sample-interval-seconds] | corpus | pur <board.pur>"
        ),
    }
    Ok(())
}
