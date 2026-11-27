// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! MothRef native entry point.
mod app;
mod gpu;
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if let Some(result) = moth_platform::update::run_helper(&args) {
        return result.map_err(Into::into);
    }
    moth_platform::diagnostics::parent_console();
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--agent")) {
        return std::thread::Builder::new()
            .name("agent".into())
            .spawn(app::agent::run)?
            .join()
            .map_err(|_| anyhow::anyhow!("agent worker panicked"))?;
    }
    if app::agent::cli::recognizes(&args) {
        let status = std::thread::Builder::new()
            .name("cli".into())
            .spawn(move || app::agent::cli::run(args))?
            .join()
            .map_err(|_| anyhow::anyhow!("CLI worker panicked"))?;
        if status != 0 {
            std::process::exit(status);
        }
        return Ok(());
    }
    // Before logging starts, so the previous run's last lines are what it wrote.
    let (session, crashed) = moth_platform::crash::begin_session();
    let (log, logging) = moth_platform::diagnostics::start()?;
    tracing_subscriber::fmt()
        .with_writer(move || log.clone())
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mothref=info,moth_render=info".into()),
        )
        .init();
    if crashed.is_some() {
        tracing::warn!("the previous run did not close normally");
    }
    let result = run(crashed);
    if let Err(error) = &result {
        tracing::error!(%error, "application stopped");
    }
    let restart = result?;
    drop(session);
    drop(logging);
    if let Some(update) = restart {
        update.commit()?;
    }
    Ok(())
}

fn run(
    crashed: Option<moth_platform::crash::Report>,
) -> anyhow::Result<Option<moth_platform::update::Armed>> {
    moth_platform::crash::install()?;
    let event_loop = winit::event_loop::EventLoop::<app::Wake>::with_user_event().build()?;
    let shutdown = event_loop.create_proxy();
    moth_platform::crash::shutdown(std::sync::Arc::new(move || {
        let _ = shutdown.send_event(app::Wake::Shutdown);
    }))?;
    let mut app = app::App::new(event_loop.create_proxy())?;
    app.crash_report = crashed;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.startup_error.take() {
        return Err(error);
    }
    Ok(app.take_update_restart())
}
