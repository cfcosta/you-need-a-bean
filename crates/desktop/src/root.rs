//! The window's one view: which page is open, the ledger it reads, and
//! the per-page choices (month, basis, horizon) a reader has made.

use std::sync::Arc;

use bean_core::model::{Day, Ledger, MonthKey};
use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyBinding, KeyContext, KeyDownEvent, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, actions, div, px,
};

use crate::{
    model::{budget::Budget, overview::Overview},
    theme::{MONO, Scheme, Theme, theme},
    ui::{self, kit, status::Status},
};

actions!(
    bean,
    [
        OpenOverview,
        OpenBudget,
        OpenReports,
        OpenInvestments,
        OpenLiabilities,
        ToggleScheme,
        OpenSearch,
        CloseSearch,
    ]
);

const CONTEXT: &str = "Bean";

/// The keys, vim-flavoured: digits open pages, `t` flips the scheme,
/// `/` searches. None of them fire while the search prompt has the keys.
pub fn bind_keys(cx: &mut App) {
    let pages = Some("Bean && !Search");
    cx.bind_keys([
        KeyBinding::new("1", OpenOverview, pages),
        KeyBinding::new("2", OpenBudget, pages),
        KeyBinding::new("3", OpenReports, pages),
        KeyBinding::new("4", OpenInvestments, pages),
        KeyBinding::new("5", OpenLiabilities, pages),
        KeyBinding::new("t", ToggleScheme, pages),
        KeyBinding::new("/", OpenSearch, pages),
        KeyBinding::new("ctrl-k", OpenSearch, Some(CONTEXT)),
        KeyBinding::new("secondary-k", OpenSearch, Some(CONTEXT)),
        KeyBinding::new("escape", CloseSearch, Some("Search")),
    ]);
}

/// What a cached budget was built for: month, basis, chosen category.
type BudgetKey = (MonthKey, u32, Option<String>);

/// What the search prompt holds.
#[derive(Clone, Debug, Default)]
pub struct Search {
    pub query: String,
    pub selected: usize,
}

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
    pub search: Option<Search>,
    /// The budget page's month; `None` follows the ledger's default.
    pub month: Option<MonthKey>,
    /// Months a "typical" is averaged over: 3, 6 or 12.
    pub basis: u32,
    /// The category the budget inspector shows; `None` picks one.
    pub category: Option<String>,
    budget: Option<(BudgetKey, Budget)>,
    /// Why the last reread of the ledger failed; the last good reading
    /// stays on screen meanwhile.
    pub reload_error: Option<String>,
    focus: FocusHandle,
}

impl Root {
    pub fn empty(cx: &mut Context<Self>) -> Self {
        Self {
            data: None,
            page: Page::Overview,
            horizon: 0,
            search: None,
            month: None,
            basis: 6,
            category: None,
            budget: None,
            reload_error: None,
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
        if self.search.is_none() {
            self.search = Some(Search::default());
        }
        cx.notify();
    }

    /// A fresh reading of the ledger, or a new day.
    pub fn replace(
        &mut self,
        ledger: Arc<Ledger>,
        today: Day,
        cx: &mut Context<Self>,
    ) {
        self.data = Some(Data::new(ledger, today));
        self.budget = None;
        self.reload_error = None;
        cx.notify();
    }

    pub fn reload_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.reload_error = Some(error);
        cx.notify();
    }

    /// The budget for the chosen month, basis and category, rebuilt only
    /// when one of them (or the ledger) changed.
    pub fn budget(&mut self) -> Option<Budget> {
        let data = self.data.as_ref()?;
        let month = self
            .month
            .unwrap_or_else(|| data.ledger.default_month(data.today));
        let key = (month, self.basis, self.category.clone());
        if let Some((k, b)) = &self.budget
            && *k == key
        {
            return Some(b.clone());
        }
        let probe = Budget::build(
            &data.ledger,
            data.today,
            month,
            self.basis,
            &data.currency,
            self.category.as_deref(),
        );
        let built = if self.category.is_none() {
            // Nothing chosen yet: show the first category that ran over,
            // else the first one there is.
            let pick = probe
                .lines
                .iter()
                .find(|l| {
                    l.account.is_some()
                        && l.status == Some(bean_core::query::Status::Over)
                })
                .or_else(|| probe.lines.iter().find(|l| l.account.is_some()))
                .and_then(|l| l.account.clone());
            Budget::build(
                &data.ledger,
                data.today,
                month,
                self.basis,
                &data.currency,
                pick.as_deref(),
            )
        } else {
            probe
        };
        self.budget = Some((key, built.clone()));
        Some(built)
    }

    pub fn set_month(&mut self, month: MonthKey, cx: &mut Context<Self>) {
        self.month = Some(month);
        cx.notify();
    }

    pub fn set_basis(&mut self, basis: u32, cx: &mut Context<Self>) {
        self.basis = basis;
        cx.notify();
    }

    pub fn select_category(&mut self, account: String, cx: &mut Context<Self>) {
        self.category = Some(account);
        cx.notify();
    }

    pub fn close_search(&mut self, cx: &mut Context<Self>) {
        self.search = None;
        cx.notify();
    }

    /// Typing while the prompt is open edits the query.
    fn type_into_search(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(search) = &mut self.search else {
            return;
        };
        let k = &event.keystroke;
        if k.modifiers.control || k.modifiers.alt || k.modifiers.platform {
            return;
        }
        match k.key.as_str() {
            "backspace" => {
                search.query.pop();
            }
            "up" => search.selected = search.selected.saturating_sub(1),
            "down" => search.selected += 1,
            _ => match &k.key_char {
                Some(text) if !text.chars().any(char::is_control) => {
                    search.query.push_str(text);
                    search.selected = 0;
                }
                _ => return,
            },
        }
        cx.stop_propagation();
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

impl Root {
    fn key_context(&self) -> KeyContext {
        let mut context = KeyContext::new_with_defaults();
        context.add(CONTEXT);
        if self.search.is_some() {
            context.add("Search");
        }
        context
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
            .track_focus(&self.focus)
            .key_context(self.key_context())
            .on_action(cx.listener(|r, _: &OpenOverview, _, cx| {
                r.open(Page::Overview, cx)
            }))
            .on_action(
                cx.listener(|r, _: &OpenBudget, _, cx| {
                    r.open(Page::Budget, cx)
                }),
            )
            .on_action(cx.listener(|r, _: &OpenReports, _, cx| {
                r.open(Page::Reports, cx)
            }))
            .on_action(cx.listener(|r, _: &OpenInvestments, _, cx| {
                r.open(Page::Investments, cx)
            }))
            .on_action(cx.listener(|r, _: &OpenLiabilities, _, cx| {
                r.open(Page::Liabilities, cx)
            }))
            .on_action(
                cx.listener(|r, _: &ToggleScheme, _, cx| r.toggle_scheme(cx)),
            )
            .on_action(cx.listener(|r, _: &OpenSearch, window, cx| {
                r.open_search(window, cx)
            }))
            .on_action(
                cx.listener(|r, _: &CloseSearch, _, cx| r.close_search(cx)),
            )
            .on_key_down(cx.listener(|r, event: &KeyDownEvent, _, cx| {
                r.type_into_search(event, cx)
            }));
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
            Page::Budget => ui::budget::page(self, &t, width, cx),
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
