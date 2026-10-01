use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};

use crate::theme::{MONO, theme};

pub struct Root {}

impl Root {
    pub fn empty(_cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for Root {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme(cx);
        div()
            .size_full()
            .bg(t.bg)
            .text_color(t.ink)
            .font_family(MONO)
            .child("")
    }
}
