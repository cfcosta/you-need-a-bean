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
