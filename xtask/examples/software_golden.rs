// SPDX-License-Identifier: GPL-3.0-or-later
fn main() -> anyhow::Result<()> {
    pollster::block_on(moth_render::headless::golden(true)).map_err(anyhow::Error::msg)
}
