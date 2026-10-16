// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! MothRef native entry point.
mod app;
mod gpu;
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mothref=info,moth_render=info".into()),
        )
        .init();
    let event_loop = winit::event_loop::EventLoop::<app::Wake>::with_user_event().build()?;
    let mut app = app::App::new(event_loop.create_proxy())?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.startup_error {
        return Err(error);
    }
    Ok(())
}
