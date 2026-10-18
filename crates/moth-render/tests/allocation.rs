// SPDX-License-Identifier: GPL-3.0-or-later
//! Retaining a fallback borrows existing mips instead of copying decoded pixels.
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn upgrading_a_preview_does_not_clone_the_full_pyramid() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let (_, device, queue) =
        pollster::block_on(moth_render::request_device(&instance, None, true)).unwrap();
    let mut registry = moth_render::Registry::new(
        &device,
        wgpu::TextureFormat::Rgba8Unorm,
        moth_io::ImageFormat::Rgba8,
    );
    let mut preview = moth_render::headless::fixture(1, 256);
    preview.original = [2048, 2048];
    let detail = moth_render::headless::fixture(1, 2048);
    registry.upload(&device, &queue, &preview);
    queue.submit([]);
    let _profiler = dhat::Profiler::builder().testing().build();
    let allocated = registry.allocated_bytes();
    registry.adapt_budget(256, moth_render::MAX_TEXTURE_BUDGET);
    assert_eq!(registry.allocated_bytes(), allocated);
    registry.upload(&device, &queue, &detail);
    queue.submit([]);
    let stats = dhat::HeapStats::get();
    dhat::assert!(
        stats.total_bytes < 8 * 1024 * 1024,
        "an upgrade must borrow mips, not clone the 21 MiB decoded pyramid"
    );
    assert!(registry.resident(detail.hash, 2048));
    assert!(registry.allocated_bytes() <= moth_render::TEXTURE_BUDGET);
    registry.adapt_budget(1, moth_render::MAX_TEXTURE_BUDGET);
    assert!(registry.allocated_bytes() <= registry.budget_bytes());
    registry.adapt_budget(0, moth_render::MAX_TEXTURE_BUDGET);
    assert_eq!(registry.allocated_bytes(), 0);
}
