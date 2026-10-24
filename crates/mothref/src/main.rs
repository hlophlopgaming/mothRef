// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! MothRef native entry point.
mod app;
mod gpu;
fn main() -> anyhow::Result<()> {
    moth_platform::diagnostics::parent_console();
    let (log, _logging) = moth_platform::diagnostics::start()?;
    tracing_subscriber::fmt()
        .with_writer(move || log.clone())
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mothref=info,moth_render=info".into()),
        )
        .init();
    let result = run();
    if let Err(error) = &result {
        tracing::error!(%error, "application stopped");
    }
    result
}
fn run() -> anyhow::Result<()> {
    moth_platform::crash::install()?;
    let event_loop = winit::event_loop::EventLoop::<app::Wake>::with_user_event().build()?;
    let shutdown = event_loop.create_proxy();
    moth_platform::crash::shutdown(std::sync::Arc::new(move || {
        let _ = shutdown.send_event(app::Wake::Shutdown);
    }))?;
    let mut app = app::App::new(event_loop.create_proxy())?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.startup_error {
        return Err(error);
    }
    Ok(())
}
