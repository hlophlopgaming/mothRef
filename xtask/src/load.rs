// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded worker load profiling; excludes native dialogs and window startup.
use moth_format::board;
use moth_platform::{
    DesktopStorage,
    boards::{Completion, Request, Worker},
};
use moth_scene::{Command, Item, Rect};
use std::{
    fs::File,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

pub fn run(path: &Path, output: Option<&Path>) -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("mothref-load-{}", moth_scene::Uuid::now_v7()));
    let storage = Arc::new(DesktopStorage::in_directory(root.join("recovery")));
    let worker = Worker::new_tab(storage.clone(), Arc::new(|| {}))?;
    let result = measure(path, output, &storage, &worker);
    let finished = worker.finish();
    let cleanup = std::fs::remove_dir_all(&root);
    result?;
    finished?;
    cleanup?;
    Ok(())
}
fn receive(worker: &Worker) -> anyhow::Result<Completion> {
    match worker.results.recv_timeout(Duration::from_secs(300))? {
        Completion::Failed(error) => anyhow::bail!(error),
        completion => Ok(completion),
    }
}
fn measure(
    path: &Path,
    output: Option<&Path>,
    storage: &Arc<DesktopStorage>,
    worker: &Worker,
) -> anyhow::Result<()> {
    let mut gpu = pollster::block_on(moth_render::headless::Harness::new(false))
        .map_err(anyhow::Error::msg)?;
    gpu.frame(0).map_err(anyhow::Error::msg)?;
    let start = Instant::now();
    let mut parse = Duration::ZERO;
    let mut durable = Duration::ZERO;
    let mut decode = Duration::ZERO;
    let mut first = None;
    let mut count = 0;
    let mut handles = Vec::new();
    if path.is_dir() {
        let mut paths = std::fs::read_dir(path)?
            .take(1025)
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()?;
        anyhow::ensure!(
            paths.len() <= 1024,
            "image directory exceeds the app import backlog (1024)"
        );
        paths.sort();
        for path in paths.into_iter().filter(|p| p.is_file()) {
            handles.push(storage.register(path)?);
        }
    } else {
        let phase = Instant::now();
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match extension {
            "moth" => {
                board::read(&mut File::open(path)?)?;
            }
            "bee" => {
                moth_import::bee::read(File::open(path)?, &mut |_| Ok(()))?;
            }
            "pur" => {
                moth_import::pur::read(File::open(path)?, &mut |_| Ok(()))?;
            }
            _ => anyhow::bail!("expected .moth, .bee, .pur or a directory of images"),
        }
        parse = phase.elapsed();
        let phase = Instant::now();
        worker.requests.send(match extension {
            "moth" => Request::Open(path.to_owned()),
            "bee" => Request::ImportBee(path.to_owned()),
            _ => Request::ImportPur(path.to_owned()),
        })?;
        handles = match receive(worker)? {
            Completion::Opened(_, sources, _) | Completion::Imported(_, sources, _) => {
                sources.into_values().collect()
            }
            _ => anyhow::bail!("expected loaded board"),
        };
        durable = phase.elapsed();
    }
    // The parse-only probe warms the input; exclude it from end-to-end timings.
    let load_start = start + parse;
    handles.sort_by_key(|handle| handle.0);
    let pool = moth_io::DecodePool::new(storage.clone(), Arc::new(|| {}))?;
    let total = handles.len();
    let mut sent = 0;
    while count < total {
        while sent < total {
            let request = moth_io::Request {
                handle: handles[sent],
                side: 256,
                format: moth_io::ImageFormat::Rgba8,
            };
            if pool.requests.try_send(request).is_err() {
                break;
            }
            sent += 1;
        }
        let phase = Instant::now();
        let completion = pool.results.recv_timeout(Duration::from_secs(300))?;
        let handle = completion.request.handle;
        let image = completion.result?;
        decode += phase.elapsed();
        if path.is_dir() {
            let phase = Instant::now();
            let item = Item::image(
                image.hash,
                Rect {
                    x: count as f32 * 260.,
                    y: 0.,
                    width: image.original[0] as f32,
                    height: image.original[1] as f32,
                },
                None,
            );
            worker
                .requests
                .send(Request::Apply(Command::Insert(item), Some(handle)))?;
            anyhow::ensure!(
                matches!(receive(worker)?, Completion::Applied(_, _)),
                "expected durable insert"
            );
            durable += phase.elapsed();
        }
        gpu.show(&image).map_err(anyhow::Error::msg)?;
        first.get_or_insert_with(|| load_start.elapsed());
        count += 1;
    }
    anyhow::ensure!(count > 0, "no images loaded");
    let full = load_start.elapsed();
    println!(
        "{} | {count} image previews | parse probe {:.3} ms | worker including parse/staging/journal {:.3} ms | decode result wait {:.3} ms | first image {:.3} ms | full previews {:.3} ms",
        path.display(),
        parse.as_secs_f64() * 1000.,
        durable.as_secs_f64() * 1000.,
        decode.as_secs_f64() * 1000.,
        first.unwrap_or_default().as_secs_f64() * 1000.,
        full.as_secs_f64() * 1000.
    );
    if let Some(output) = output {
        worker.requests.send(Request::Save(output.to_owned()))?;
        anyhow::ensure!(
            matches!(receive(worker)?, Completion::Saved),
            "expected save"
        );
        board::read(&mut File::open(output)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn profiles_durable_import_and_gpu_preview() {
        super::run(
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../corpus/bee/tiny.bee"
            )),
            None,
        )
        .unwrap();
    }
}
