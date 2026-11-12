use super::{
    actions,
    view::{button, copy, heading, source},
};
use crate::{shell::MainState, theme::Theme};
use kudzu_app::onboarding::ImportSource;
use rune::{col, row, El};

pub fn welcome(s: &MainState, t: &Theme) -> El {
    let intro = col()
        .w_full()
        .items_center()
        .gap(8.0)
        .child(
            heading("For things worth repeating", t)
                .font(t.ty.headline_large)
                .w(321.0)
                .align(rune::Align::Center),
        )
        .child(
            copy("Clipboard history and text shortcuts for your Mac.", t)
                .w_full()
                .align(rune::Align::Center)
                .font(rune::TextStyle::new(14.0, 400.0).lh(22.75)),
        );
    let signoff = col()
        .child(copy("Best regards,", t).font(rune::TextStyle::new(12.0, 400.0).lh(19.5)))
        .child(
            copy("Your Name", t)
                .font(rune::TextStyle::new(12.0, 400.0).lh(19.5))
                .color(t.c.foreground),
        );
    let example = t.depth.card.iter().fold(
        row()
            .h(65.0)
            .w_full()
            .px(17.0)
            .py(13.0)
            .gap(12.0)
            .items_center()
            .radius(12.0)
            .fill(t.c.surface.alpha(0.7))
            .child(
                copy(";sig", t)
                    .font(t.ty.mono)
                    .px(8.0)
                    .py(4.0)
                    .radius(6.0)
                    .fill(t.c.iris_soft)
                    .color(t.c.iris_foreground),
            )
            .child(rune::icon(&crate::theme::icons::ARROW_RIGHT, 14.0).color(t.c.faint))
            .child(signoff),
        |el, shadow| el.box_shadow(*shadow),
    );
    col()
        .gap(24.0)
        .center()
        .child(
            s.onboarding
                .brand
                .as_ref()
                .map_or_else(
                    || {
                        crate::widgets::wordmark::wordmark(
                            crate::widgets::wordmark::Variant::Full,
                            61.25,
                            t.c.foreground,
                        )
                    },
                    |brand| brand.view(t),
                )
                .mb(8.0),
        )
        .child(intro)
        .child(example)
        .child(button("Get started", "onboarding-start", false, t, actions::next).h(40.0))
        .child(
            copy("A few small steps, then back to your words.", t)
                .font(rune::TextStyle::new(11.0, 400.0).lh(20.0))
                .color(t.c.faint)
                .mt(-12.0),
        )
}
pub fn access(s: &MainState, t: &Theme) -> El {
    let intro = col()
        .gap(8.0)
        .child(heading("Let Kudzu type for you", t))
        .child(copy(
            "Allow access once in macOS. We’ll guide you through it and detect when you’re ready.",
            t,
        ));
    let ready = s.svc.onboarding.can_continue();
    let card = super::access::card(s, t);
    let nav = row()
        .w_full()
        .justify_between()
        .child(button(
            "Skip for now",
            "onboarding-skip",
            false,
            t,
            actions::skip,
        ))
        .child(button(
            "Continue",
            "onboarding-continue",
            !ready,
            t,
            actions::next,
        ));
    col().gap(24.0).child(intro).child(card).child(nav)
}
pub fn import(s: &MainState, t: &Theme) -> El {
    let m = s.svc.onboarding.snapshot();
    let intro = col().gap(8.0).child(heading("Bring your snippets?", t))
        .child(copy("Switching from another expander? Forms, dates and clipboard variables come along; anything Kudzu can’t run yet is kept as text. A backup is saved first.", t));
    let sources = col()
        .role("list")
        .label("Import sources")
        .gap(8.0)
        .child(source(s, t, ImportSource::Espanso, "Espanso"))
        .child(source(s, t, ImportSource::Raycast, "Raycast"))
        .child(source(s, t, ImportSource::TextExpander, "TextExpander"));
    let nav = if m.imported > 0 {
        button(
            "Continue",
            "onboarding-continue",
            m.busy(),
            t,
            actions::next,
        )
    } else {
        button(
            "Skip for now",
            "onboarding-skip",
            m.busy(),
            t,
            actions::skip,
        )
    };
    col()
        .gap(24.0)
        .child(intro)
        .child(sources)
        .child(row().w_full().justify_end().child(nav))
}
