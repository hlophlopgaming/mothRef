// SPDX-License-Identifier: GPL-3.0-or-later
fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/moth.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("../../assets/icons/moth.ico")
        .compile()
        .expect("the bundled application icon must compile");
}
