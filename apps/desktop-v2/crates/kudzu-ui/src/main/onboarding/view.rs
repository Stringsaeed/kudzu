use super::actions;
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::wordmark::{wordmark, Variant as Mark};
use kudzu_app::onboarding::{Detection, ImportSource, Step};
use rune::{col, row, text, text_input, El};
pub(super) fn copy(value: impl Into<String>, t: &Theme) -> El {
    text(value)
        .wrap()
        .font(rune::TextStyle::new(14.0, 400.0).lh(20.0))
        .color(t.c.muted_foreground)
}
pub(super) fn heading(value: &str, t: &Theme) -> El {
    text(value)
        .role("heading")
        .label(value)
        .font(rune::TextStyle::new(20.0, 600.0).lh(28.0))
        .color(t.c.foreground)
}
pub(super) fn button(
    label: &str,
    key: &str,
    disabled: bool,
    t: &Theme,
    action: fn(&mut MainState),
) -> El {
    let ghost = label == "Skip" || label == "Skip for now";
    let mut control = Button::new(label).variant(if ghost {
        Variant::Ghost
    } else if label == "Review permissions" {
        Variant::Outline
    } else {
        Variant::Default
    });
    if matches!(label, "Continue" | "Get started" | "Open library") {
        control = control.icon_end(&icons::ARROW_RIGHT);
    }
    if label == "Get started" {
        control = control.icon_end(&icons::ARROW_RIGHT);
    }
    let mut el = control
        .disabled(disabled)
        .on_press(action)
        .build(t)
        .key(key);
    if label == "Get started" {
        el = el.w(138.703).px(20.0);
    }
    el
}
pub fn view(s: &MainState, t: &Theme) -> El {
    let m = s.svc.onboarding.snapshot();
    let busy = m.busy() || s.onboarding.opening();
    let label = match m.step {
        Step::Welcome => "Welcome",
        Step::Access => "Access",
        Step::Import => "Import",
        Step::TryIt => "Try it",
    };
    let content = match m.step {
        Step::Welcome => super::steps::welcome(s, t),
        Step::Access => super::steps::access(s, t),
        Step::Import => super::steps::import(s, t),
        Step::TryIt => practice(s, t),
    };
    let mut card = col()
        .key("onboarding-card")
        .role("region")
        .label(format!("Setup: {label}"))
        .w_full()
        .max_w(480.0)
        .px(24.0)
        .py(96.0)
        .gap(32.0)
        .no_window_drag()
        .child(
            content.key(format!("onboarding-step-{label}")).transition(
                rune::Transition::offset(f32::from(m.direction) * 12.0, 0.0)
                    .combined(rune::Transition::opacity())
                    .springs(rune::SpringPair::new(
                        crate::theme::motion::soft_spring(0.24),
                        crate::theme::motion::soft_spring(0.24),
                    )),
            ),
        );
    let error = m.error.as_deref().or(if s.onboarding.scene_finish_error {
        Some(kudzu_app::onboarding::FINISH_ERROR)
    } else {
        None
    });
    if let Some(e) = error {
        card = card.child(
            text(e)
                .wrap()
                .role("alert")
                .font(t.ty.body)
                .color(t.c.destructive)
                .padding(12.0)
                .h(40.0),
        );
    }
    let mut progress = row()
        .key("onboarding-progress")
        .role("list")
        .label("Setup progress")
        .gap(20.0)
        .center();
    for (step, title) in [
        (Step::Welcome, "Welcome"),
        (Step::Access, "Access"),
        (Step::Import, "Import"),
        (Step::TryIt, "Try it"),
    ] {
        let mark = if step < m.step {
            rune::icon(&icons::CHECK, 12.0).color(t.c.iris)
        } else {
            rune::el()
                .size(6.0, 6.0)
                .radius(3.0)
                .fill(if step == m.step {
                    t.c.iris
                } else {
                    t.c.foreground.alpha(0.15)
                })
        };
        progress = progress.child(
            row()
                .key(format!("onboarding-progress-{title}"))
                .role("listitem")
                .gap(6.0)
                .items_center()
                .child(col().size(12.0, 12.0).center().child(mark))
                .child(text(title).font(t.ty.meta_10).color(if step == m.step {
                    t.c.foreground
                } else {
                    t.c.faint
                })),
        );
    }
    let mut root = col()
        .key("onboarding-view-slot")
        .w_full()
        .h_full()
        .center()
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        })
        .fill(t.c.background)
        .radial_gradient(
            [0.5, 0.3],
            [590.0, 532.0],
            [(0.0, t.c.iris.alpha(0.07)), (0.6, t.c.iris.alpha(0.0))],
        )
        .window_drag()
        .child(card.child(progress));
    if m.step != Step::Welcome {
        root = root
            .child(wordmark(Mark::Small, 22.35, t.c.muted_foreground).absolute(32.0, 56.0))
            .child(
                Button::new("Back")
                    .size(Size::Sm)
                    .icon(&icons::ARROW_LEFT)
                    .variant(Variant::Ghost)
                    .disabled(busy)
                    .on_press(actions::back)
                    .build(t)
                    .key("onboarding-back")
                    .pin(None, Some(56.0), Some(32.0), None)
                    .h(28.0),
            );
    }
    root
}
fn practice(s: &MainState, t: &Theme) -> El {
    let m = s.svc.onboarding.snapshot();
    let field = text_input(
        &s.onboarding.practice,
        rune::TextStyle::new(15.0, 400.0).lh(20.0),
    )
    .key("onboarding-practice")
    .label("Practice field")
    .w_full()
    .h(112.0)
    .padding(16.0)
    .radius(12.0)
    .color(t.c.foreground)
    .disabled(m.busy() || s.onboarding.opening())
    .placeholder("Type ;kudzu here…", t.c.faint)
    .on_edit(|s: &mut MainState| &mut s.onboarding.practice)
    .on_input_change(|s: &mut MainState, _| s.onboarding.substitute());
    let intro = col()
        .gap(8.0)
        .child(heading("Try a preview", t))
        .child(copy("Type ;kudzu below to see how expansion feels.", t));
    let mut surface = t.depth.card.iter().fold(
        col().w_full().radius(12.0).fill(t.c.surface).child(field),
        |el, shadow| el.box_shadow(*shadow),
    );
    if s.onboarding.success() {
        surface = surface.border(1.0, t.c.success).box_shadow(rune::Shadow {
            x: 0.0,
            y: 0.0,
            blur: 24.0,
            spread: -6.0,
            color: t.c.success,
            inset: false,
        });
    }
    let mut v = col().gap(24.0).child(intro).child(surface);
    if s.onboarding.success() {
        let reminder = row()
            .gap(6.0)
            .child(copy("Forgot a trigger? Press", t))
            .child(
                crate::widgets::shortcut::Shortcut::accelerator(
                    &s.svc.settings.watch().get().palette_hotkey,
                )
                .build(t),
            );
        let explanation = "When expansion is enabled, your library’s triggers work in Notes, Mail, your browser, and other apps.";
        v = v
            .child(
                copy("✦ That’s how a snippet expands.", t)
                    .key("onboarding-celebration")
                    .transition(crate::motion::check_in()),
            )
            .child(copy(explanation, t))
            .child(reminder)
            .child(copy("anywhere to search your snippets.", t));
    }
    v.child(row().w_full().justify_end().child(button(
        if m.finishing || s.onboarding.opening() {
            "Opening…"
        } else if s.onboarding.success() {
            "Open library"
        } else {
            "Skip"
        },
        if s.onboarding.success() {
            "onboarding-open-library"
        } else {
            "onboarding-finish"
        },
        m.busy() || s.onboarding.opening(),
        t,
        actions::finish,
    )))
}
pub(super) fn source(s: &MainState, t: &Theme, source: ImportSource, name: &str) -> El {
    let m = s.svc.onboarding.snapshot();
    let detail = match source {
        ImportSource::Espanso => match &m.detection {
            Detection::Looking => "Looking on this Mac…".into(),
            Detection::NotFound if m.outcomes.contains_key(&source) => {
                "From the folder you chose.".into()
            }
            Detection::NotFound => {
                "Not found on this Mac. If you keep it elsewhere, choose its folder.".into()
            }
            Detection::Failed => {
                "Couldn’t check this Mac. Choose your Espanso folder instead.".into()
            }
            Detection::Found { folders } => format!(
                "Found {} snippets on this Mac.",
                folders.iter().map(|f| f.snippet_count).sum::<usize>()
            ),
        },
        ImportSource::Raycast => {
            "In Raycast, run Export Snippets, then choose the .json file.".into()
        }
        ImportSource::TextExpander => {
            "In TextExpander, export a group as .textexpander or CSV.".into()
        }
    };
    let detected =
        source == ImportSource::Espanso && matches!(m.detection, Detection::Found { .. });
    let imported =
        detected && m.outcomes.contains_key(&source) && !m.failures.contains_key(&source);
    let label = if imported {
        "Imported"
    } else if detected {
        if m.failures.contains_key(&source) {
            "Retry"
        } else {
            "Import"
        }
    } else if source == ImportSource::Espanso {
        "Choose folder…"
    } else if m.failures.contains_key(&source) {
        "Try again…"
    } else {
        "Choose file…"
    };
    let accessible = if detected {
        format!("{label} Espanso snippets")
    } else {
        format!("{label} for {name}")
    };
    let action = Button::new(if m.working == Some(source) {
        "Importing…"
    } else {
        label
    })
    .size(Size::Sm)
    .variant(if detected {
        Variant::Default
    } else {
        Variant::Secondary
    })
    .disabled(
        m.busy()
            || imported
            || (source == ImportSource::Espanso && m.detection == Detection::Looking),
    )
    .on_press_tx(move |s: &mut MainState, tx| {
        if detected {
            actions::detected(s)
        } else {
            actions::pick(s, tx, source)
        }
    })
    .build(t)
    .key(actions::source_key(source))
    .label(accessible)
    .h(28.0)
    .min_w(112.0);
    let mut content = col().gap(8.0).child(
        row()
            .w_full()
            .items_center()
            .justify_between()
            .gap(16.0)
            .child(
                col()
                    .min_w(0.0)
                    .grow(1.0)
                    .shrink()
                    .gap(2.0)
                    .child(copy(name, t))
                    .child(copy(detail, t).font(rune::TextStyle::new(12.0, 400.0).lh(16.0))),
            )
            .child(action),
    );
    if let Some(o) = m.outcomes.get(&source) {
        content = content.child(
            copy(format!("Imported {} snippets", o.count), t)
                .font(rune::TextStyle::new(12.0, 400.0).lh(16.0)),
        );
        if o.changed > 0 {
            content = content.child(
                copy(
                    format!(
                        "{} items need a look — e.g. {}",
                        o.changed,
                        o.first_change
                            .as_deref()
                            .unwrap_or("Review the imported snippets.")
                    ),
                    t,
                )
                .font(rune::TextStyle::new(12.0, 400.0).lh(16.0)),
            );
        }
    }
    if let Some(e) = m.failures.get(&source) {
        content = content.child(
            text(e)
                .wrap()
                .role("alert")
                .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                .color(t.c.destructive),
        );
    }
    content
        .role("listitem")
        .w_full()
        .px(17.0)
        .py(13.0)
        .radius(12.0)
        .fill(t.c.surface.alpha(0.7))
}
