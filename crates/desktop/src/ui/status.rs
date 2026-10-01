//! The status line across the top: the bean, the five numbered pages,
//! the ledger, search and today — a vim statusline more than a navbar.

use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, prelude::*, px,
};

use crate::{
    root::{Page, Root},
    theme::Theme,
};

pub const HEIGHT: f32 = 34.;

pub struct Status<'a> {
    pub page: Page,
    /// A step below the page, such as the account a register shows.
    pub crumb: Option<SharedString>,
    pub ledger: SharedString,
    pub today: SharedString,
    pub narrow: bool,
    pub extra: Option<gpui::AnyElement>,
    pub t: &'a Theme,
}

impl Status<'_> {
    pub fn render(self, cx: &mut Context<Root>) -> gpui::AnyElement {
        let t = self.t;
        let mut tabs = div().flex().flex_row().items_stretch();
        for (i, page) in Page::TABS.iter().enumerate() {
            let on = *page == self.page;
            let target = *page;
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("tab-{}", page.name())))
                    .flex()
                    .items_center()
                    .px(px(14.))
                    .text_color(if on { t.ink } else { t.dim })
                    .when(on, |d| d.bg(t.sel))
                    .when(!on, |d| d.hover(|s| s.bg(t.hl).text_color(t.ink)))
                    .cursor_pointer()
                    .on_click(
                        cx.listener(move |root, _, _, cx| {
                            root.open(target, cx)
                        }),
                    )
                    .child(
                        div()
                            .text_color(if on { t.blue } else { t.mut_ })
                            .child(format!("{}", i + 1)),
                    )
                    .child(format!(" {}", page.name())),
            );
            if on && let Some(crumb) = self.crumb.clone() {
                tabs = tabs.child(
                    div()
                        .flex()
                        .items_center()
                        .px(px(14.))
                        .bg(t.hl)
                        .text_color(t.ink2)
                        .child(format!("› {crumb}")),
                );
            }
        }

        div()
            .flex()
            .flex_row()
            .flex_none()
            .h(px(HEIGHT))
            .bg(t.bar)
            .border_b_1()
            .border_color(t.line)
            .whitespace_nowrap()
            .child(
                div()
                    .id("bean")
                    .flex()
                    .items_center()
                    .px(px(14.))
                    .bg(t.yellow)
                    .text_color(t.on_block)
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .on_click(cx.listener(|root, _, _, cx| {
                        root.open(Page::Overview, cx)
                    }))
                    .child("◆ bean"),
            )
            .when(!self.narrow, |d| d.child(tabs))
            .child(div().flex_1())
            .children(self.extra)
            .when(!self.narrow, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .px(px(14.))
                        .text_color(t.dim)
                        .child(self.ledger.clone()),
                )
            })
            .child(
                div()
                    .id("search")
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .border_l_1()
                    .border_color(t.line)
                    .text_color(t.ink2)
                    .cursor_pointer()
                    .on_click(cx.listener(|root, _, window, cx| {
                        root.open_search(window, cx)
                    }))
                    .child("/ search")
                    .child(div().text_color(t.mut_).child("⌘K")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(px(14.))
                    .bg(t.blue)
                    .text_color(t.on_block)
                    .font_weight(FontWeight::BOLD)
                    .child(self.today),
            )
            .into_any_element()
    }
}

/// The 3/6/12-month basis, sat in the status line on the pages whose
/// figures it changes.
pub fn basis(
    t: &Theme,
    basis: u32,
    cx: &mut Context<Root>,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .items_stretch()
        .border_l_1()
        .border_r_1()
        .border_color(t.line2)
        .children([3u32, 6, 12].into_iter().enumerate().map(|(i, b)| {
            let on = b == basis;
            div()
                .id(SharedString::from(format!("status-basis-{b}")))
                .flex()
                .items_center()
                .px(px(12.))
                .when(i > 0, |d| d.border_l_1().border_color(t.line2))
                .when(on, |d| d.bg(t.sel))
                .text_color(if on { t.ink } else { t.dim })
                .cursor_pointer()
                .on_click(cx.listener(move |r, _, _, cx| r.set_basis(b, cx)))
                .child(b.to_string())
        }))
        .into_any_element()
}
