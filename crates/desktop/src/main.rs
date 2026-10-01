use std::{sync::Arc, time::Duration};

use bean_core::{
    loader::load,
    model::{Day, Ledger},
    watch::fingerprint,
};
use bean_desktop::{Root, cli, follow::Settle};
use gpui::{
    App, AppContext, Bounds, Focusable, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};

/// How often the ledger's files are looked at for edits.
const POLL: Duration = Duration::from_millis(500);

fn local_today() -> Day {
    let d = jiff::Zoned::now().date();
    (d.year() as u16, d.month() as u8, d.day() as u8)
}

fn main() {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let ledger = match load(&options.ledger) {
        Ok(loaded) => Arc::new(Ledger::build(loaded)),
        Err(error) => {
            eprintln!("you-need-a-bean: {}", error.summary());
            std::process::exit(1);
        }
    };
    let pinned = options.today;
    let today = pinned.unwrap_or_else(local_today);
    let path = options.ledger.clone();
    let title = ledger
        .title
        .clone()
        .unwrap_or_else(|| "you need a bean".into());

    gpui_platform::application().run(move |cx: &mut App| {
        bean_desktop::init(cx);
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("{title} — you need a bean").into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(360.), px(560.))),
                app_id: Some("you-need-a-bean".into()),
                ..Default::default()
            },
            |window, cx| {
                let root = cx.new(|cx| Root::new(ledger.clone(), today, cx));
                let focus = root.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
                root
            },
        );
        let Ok(root) = window.and_then(|w| w.entity(cx)) else {
            eprintln!("you-need-a-bean: could not open a window");
            cx.quit();
            return;
        };

        // Follow the files, and the date, for as long as the app runs.
        cx.spawn(async move |cx| {
            let files = |cx: &mut gpui::AsyncApp| {
                root.read_with(cx, |r, _| {
                    r.data
                        .as_ref()
                        .map(|d| d.ledger.files.clone())
                        .unwrap_or_default()
                })
            };
            let mut settle = Settle::new(fingerprint(&files(cx)));
            let mut day = today;
            loop {
                cx.background_executor().timer(POLL).await;
                let now = pinned.unwrap_or_else(local_today);
                let seen = fingerprint(&files(cx));
                if settle.tick(seen) {
                    let path = path.clone();
                    let read = cx
                        .background_spawn(async move {
                            load(&path).map(Ledger::build)
                        })
                        .await;
                    match read {
                        Ok(ledger) => root.update(cx, |r, cx| {
                            r.replace(Arc::new(ledger), now, cx)
                        }),
                        Err(error) => root.update(cx, |r, cx| {
                            r.reload_failed(error.summary(), cx)
                        }),
                    }
                    settle.settle(fingerprint(&files(cx)));
                    day = now;
                } else if now != day {
                    day = now;
                    root.update(cx, |r, cx| {
                        if let Some(ledger) =
                            r.data.as_ref().map(|d| d.ledger.clone())
                        {
                            r.replace(ledger, now, cx);
                        }
                    });
                }
            }
        })
        .detach();
        cx.activate(true);
    });
}
