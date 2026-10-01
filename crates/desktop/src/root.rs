//! The window's one view: which page is open, the ledger it reads, and
//! the per-page choices (month, basis, horizon) a reader has made.

use std::sync::Arc;

use bean_core::model::{Day, Ledger};
use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use crate::{
    model::overview::Overview,
    theme::{MONO, Scheme, Theme, theme},
    ui::{self, kit, status::Status},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Overview,
    Budget,
    Reports,
    Investments,
    Liabilities,
}

impl Page {
    pub const TABS: [Page; 5] = [
        Page::Overview,
        Page::Budget,
        Page::Reports,
        Page::Investments,
        Page::Liabilities,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Page::Overview => "overview",
            Page::Budget => "budget",
            Page::Reports => "reports",
            Page::Investments => "invest",
            Page::Liabilities => "debts",
        }
    }
}

/// One reading of the ledger and the figures derived from it.
pub struct Data {
    pub ledger: Arc<Ledger>,
    pub today: Day,
    pub currency: String,
    pub overview: Overview,
}

impl Data {
    pub fn new(ledger: Arc<Ledger>, today: Day) -> Self {
        let currency = ledger
            .operating_currencies
            .first()
            .cloned()
            .unwrap_or_else(|| "USD".into());
        let overview = Overview::build(&ledger, today, &currency);
        Self {
            ledger,
            today,
            currency,
            overview,
        }
    }

    /// `main.beancount · 119`: the root file and how many directives.
    pub fn ledger_name(&self) -> SharedString {
        let file = self
            .ledger
            .files
            .first()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ledger".into());
        format!("{file} · {}", self.ledger.directives).into()
    }
}

pub struct Root {
    pub data: Option<Data>,
    pub page: Page,
    /// 30, 60 or 90 days: which forecast the overview draws.
    pub horizon: usize,
    focus: FocusHandle,
}

impl Root {
    pub fn empty(cx: &mut Context<Self>) -> Self {
        Self {
            data: None,
            page: Page::Overview,
            horizon: 0,
            focus: cx.focus_handle(),
        }
    }

    pub fn new(
        ledger: Arc<Ledger>,
        today: Day,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            data: Some(Data::new(ledger, today)),
            ..Self::empty(cx)
        }
    }

    pub fn open(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    pub fn open_search(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
    }

    pub fn set_horizon(&mut self, horizon: usize, cx: &mut Context<Self>) {
        self.horizon = horizon;
        cx.notify();
    }

    pub fn toggle_scheme(&mut self, cx: &mut Context<Self>) {
        let next = theme(cx).scheme.toggled();
        cx.set_global(Theme::of(next));
        cx.notify();
    }

    pub fn scheme(cx: &App) -> Scheme {
        theme(cx).scheme
    }
}

impl Focusable for Root {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Root {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme(cx).clone();
        let width = f32::from(window.viewport_size().width);
        let base = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg)
            .text_color(t.ink)
            .font_family(MONO)
            .text_size(px(kit::SIZE))
            .line_height(px(kit::LINE))
            .track_focus(&self.focus);
        let Some(data) = &self.data else {
            return base;
        };
        let status = Status {
            page: self.page,
            crumb: None,
            ledger: data.ledger_name(),
            today: bean_core::home::date(data.today).into(),
            narrow: width < 640.,
            extra: None,
            t: &t,
        }
        .render(cx);
        let page = match self.page {
            Page::Overview => ui::overview::page(self, &t, width, cx),
            _ => kit::any(div()),
        };
        base.child(status).child(
            div()
                .id("page")
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .child(page),
        )
    }
}
