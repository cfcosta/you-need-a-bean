//! Renders views offscreen through gpui's headless wgpu renderer, with
//! only the fonts the app embeds, so a picture does not depend on the
//! machine it was taken on.

#![allow(dead_code)]

use std::sync::Arc;

use gpui::{
    AnyWindowHandle, AppContext, HeadlessAppContext, Render, Size, Window, px,
    size,
};
use image::RgbaImage;

pub fn headless() -> HeadlessAppContext {
    let text = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        bean_desktop::theme::MONO,
    ));
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), || {
        gpui_platform::current_headless_renderer()
    });
    cx.update(bean_desktop::init);
    cx
}

pub fn frame(width: f32, height: f32) -> Size<gpui::Pixels> {
    size(px(width), px(height))
}

/// Opens `view` in a window of `size` and takes its picture.
pub fn shoot<V: Render + 'static>(
    cx: &mut HeadlessAppContext,
    size: Size<gpui::Pixels>,
    view: impl FnOnce(&mut Window, &mut gpui::Context<V>) -> V + 'static,
) -> RgbaImage {
    let window = cx
        .open_window(size, |window, cx| cx.new(|cx| view(window, cx)))
        .expect("a headless window");
    cx.run_until_parked();
    let handle: AnyWindowHandle = window.into();
    cx.capture_screenshot(handle).expect("a screenshot")
}

pub fn pixel(image: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let p = image.get_pixel(x, y).0;
    [p[0], p[1], p[2]]
}

/// How far a picture is from its canvas reference: the share of pixels
/// whose colour moved by more than a glyph's anti-aliasing would explain.
/// Writes the picture and a diff (mismatches in red over a faded copy of
/// the reference) to `target/visual/` for a person to look at.
pub fn mismatch(shot: &RgbaImage, reference: &str) -> f64 {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let expected =
        image::open(dir.join(format!("tests/reference/{reference}.png")))
            .expect("a reference picture")
            .to_rgba8();
    let out = dir.join("../../target/visual");
    std::fs::create_dir_all(&out).unwrap();
    shot.save(out.join(format!("{reference}-actual.png")))
        .unwrap();

    let (w, h) = (
        expected.width().min(shot.width()),
        expected.height().min(shot.height()),
    );
    let mut diff = RgbaImage::new(w, h);
    let mut off = 0u64;
    for y in 0..h {
        for x in 0..w {
            let a = shot.get_pixel(x, y).0;
            let b = expected.get_pixel(x, y).0;
            let delta = (0..3).map(|i| a[i].abs_diff(b[i])).max().unwrap();
            if delta > 48 {
                off += 1;
                diff.put_pixel(x, y, image::Rgba([255, 0, 64, 255]));
            } else {
                let g = |c: u8| 128 + c / 2;
                diff.put_pixel(
                    x,
                    y,
                    image::Rgba([g(b[0]), g(b[1]), g(b[2]), 255]),
                );
            }
        }
    }
    diff.save(out.join(format!("{reference}-diff.png")))
        .unwrap();
    let size_penalty = if (shot.width(), shot.height())
        == (expected.width(), expected.height())
    {
        0.0
    } else {
        1.0
    };
    off as f64 / f64::from(w * h) + size_penalty
}
