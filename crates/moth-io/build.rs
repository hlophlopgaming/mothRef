// SPDX-License-Identifier: GPL-3.0-or-later
//! Links the C++ runtime that `intel_tex_2` needs.
//!
//! `intel_tex_2` (x86/x86_64 only) links its ASTC C++ object unconditionally,
//! which references the C++ exception personality even though MothRef only calls
//! the BC7 path. MSVC links its C++ runtime automatically; Unix needs it explicit.
fn main() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
        let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if family == "unix" {
            println!(
                "cargo:rustc-link-lib={}",
                if os == "macos" { "c++" } else { "stdc++" }
            );
        }
    }
}
