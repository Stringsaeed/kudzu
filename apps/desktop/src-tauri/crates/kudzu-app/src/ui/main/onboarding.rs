//! First-run onboarding (`components/app/main/onboarding.tsx`): welcome, guided permissions,
//! importing from another expander, and a practice field, over the iris atmosphere.
//!
//! ```text
//! ┌ bg-background + radial iris 7 % at (50 %, 30 %) ────────────┐
//! │ wordmark (hidden on welcome)                       ← Back   │  top 56, px 32
//! │                ┌ max-w 480, px 24, py 96, gap 32 ┐          │  centred vertically
//! │                │ step (animate-rise)             │          │
//! │                │ error                           │          │
//! │                │ • Welcome  • Access  • Import … │          │
//! │                └─────────────────────────────────┘          │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use rune::{col, icon, row, text, text_input, Align, Beat, Color, Cx, El, FileDialogOptions, LoopSpec, Sequence, Shadow, ScrollSpec, Spring, SpringPair, TextEdit, TextStyle, Transition, Tx};

use kudzu_core::interchange::coordinator::ImportSummary;

use crate::backend::commands::{self, EspansoDetection};
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::shortcut::{accelerator_labels, Shortcut, ShortcutSize};
use crate::ui::kit::wordmark::{wordmark, WordmarkVariant};
use crate::ui::main::insights::{still, whole};
use crate::ui::main::MainUi;
use crate::ui::permissions::{check_in, permissions_ready, setup_card};
use crate::ui::theme::{motion, radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

const TAGLINE: &str = "For things worth repeating";
const DESCRIPTION: &str = "Clipboard history and text shortcuts for your Mac.";
const SAMPLE_TRIGGER: &str = ";kudzu";
const SAMPLE_TEXT: &str = "Hello from Kudzu!";
/// `min-w-28`.
const ACTION_WIDTH: f32 = 112.0;
/// See the practice field: the web never shows its success glow.
const SUCCESS_GLOW: bool = false;
const RAYCAST_HINT: &str = "In Raycast, run Export Snippets, then choose the .json file.";
const TEXTEXPANDER_HINT: &str = "In TextExpander, export a group as .textexpander or CSV.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Step {
    #[default]
    Welcome,
    Permissions,
    Import,
    Practice,
}

impl Step {
    fn label(self) -> &'static str {
        match self {
            Step::Welcome => "Welcome",
            Step::Permissions => "Access",
            Step::Import => "Import",
            Step::Practice => "Try it",
        }
    }
}

/// macOS needs the permissions step (`platform === "macos"`).
const STEPS: [Step; 4] = [Step::Welcome, Step::Permissions, Step::Import, Step::Practice];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Espanso,
    Raycast,
    TextExpander,
}

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    count: usize,
    changed: usize,
    first_change: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Detection {
    #[default]
    Looking,
    Done,
    Failed,
}

#[derive(Default)]
pub struct OnboardingState {
    pub step: Step,
    started: Cell<bool>,
    espanso: Vec<EspansoDetection>,
    detection: Detection,
    completed: HashSet<PathBuf>,
    working: Option<Source>,
    picking: bool,
    outcomes: HashMap<Source, Outcome>,
    failures: HashMap<Source, String>,
    /// Running total of new snippets: decides Continue vs. Skip.
    imported: usize,
    finishing: bool,
    error: Option<String>,
    practice: TextEdit,
}

impl OnboardingState {
    fn busy(&self) -> bool {
        self.finishing || self.picking || self.working.is_some()
    }
    fn index(&self) -> usize {
        STEPS.iter().position(|s| *s == self.step).unwrap_or(0)
    }
}

/// `animate-rise` (`rise`, `--motion-slow` on `--spring-soft`).
fn rise() -> Transition {
    Transition::offset(0.0, 4.0).combined(Transition::opacity()).springs(SpringPair::new(motion::soft(), Spring::NONE))
}

/// `animate-star` (`star-burst`, 420 ms ease-out): 0.5 → 1.25 → 1.
const STAR: &[Beat] = &[Beat::new(0.0).scale(0.5), Beat::new(0.231).scale(1.25).easing(rune::Easing::EASE_OUT), Beat::new(0.42).easing(rune::Easing::EASE_OUT)];

/// Snippets a result really added: the source's count minus what deduplication merged away.
fn new_snippets(r: &ImportSummary) -> usize {
    r.imported_snippets.saturating_sub(r.dedupe_report.merged_snippets)
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// MARK: Actions

fn next(s: &mut MainUi) {
    let i = s.onboarding.index();
    if i + 1 < STEPS.len() {
        go(s, STEPS[i + 1]);
    } else {
        finish(s);
    }
}

fn go(s: &mut MainUi, step: Step) {
    s.onboarding.step = step;
    if step == Step::Practice {
        s.onboarding.practice.focused = true;
    }
}

fn finish(s: &mut MainUi) {
    s.onboarding.finishing = true;
    s.onboarding.error = None;
    s.spawn(
        || commands::update_settings(serde_json::json!({ "onboardingCompleted": true })),
        |s, result, _| {
            s.onboarding.finishing = false;
            match result {
                Ok(saved) => s.settings = Arc::new(saved),
                Err(_) => s.onboarding.error = Some("We couldn\u{2019}t finish setup. Your library is safe; please try again.".into()),
            }
        },
    );
}

/// Each source shows its latest result: how many snippets were new, not how many were in the file.
fn record(s: &mut MainUi, source: Source, r: &ImportSummary, accumulate: bool) {
    let ob = &mut s.onboarding;
    let added = new_snippets(r);
    ob.imported += added;
    let before = if accumulate { ob.outcomes.get(&source).cloned() } else { None }.unwrap_or_default();
    let first = before.first_change.clone().or_else(|| r.warnings.first().map(|w| w.message.clone()));
    ob.outcomes.insert(source, Outcome { count: before.count + added, changed: before.changed + r.warnings.len(), first_change: first });
}

fn fail(s: &mut MainUi, source: Source, e: &str, prefix: &str) {
    let msg = e.strip_prefix("Error:").map(str::trim_start).unwrap_or(e);
    s.onboarding.failures.insert(source, format!("{prefix}{msg}"));
}

fn begin(s: &mut MainUi, source: Source) {
    s.onboarding.working = Some(source);
    s.onboarding.failures.remove(&source);
}

fn detect(s: &MainUi) {
    if s.onboarding.started.replace(true) {
        return;
    }
    // Detection can't return an error, but a scan that dies counts as one (the web's rejection).
    s.spawn(
        || std::panic::catch_unwind(commands::detect_espanso),
        |s, found, _| match found {
            Ok(found) => {
                s.onboarding.espanso = found;
                s.onboarding.detection = Detection::Done;
            }
            Err(_) => s.onboarding.detection = Detection::Failed,
        },
    );
}

/// Imports every detected Espanso folder not yet imported, stopping at the first failure.
fn import_detected(s: &mut MainUi) {
    begin(s, Source::Espanso);
    let todo: Vec<PathBuf> = s.onboarding.espanso.iter().map(|d| d.path.clone()).filter(|p| !s.onboarding.completed.contains(p)).collect();
    s.spawn(
        move || {
            let mut done = Vec::new();
            for path in todo {
                match commands::import_espanso(path.clone()) {
                    Ok(r) => done.push((path, r)),
                    Err(e) => return (done, Some(e)),
                }
            }
            (done, None)
        },
        |s, (done, err): (Vec<(PathBuf, ImportSummary)>, Option<String>), _| {
            for (path, r) in &done {
                record(s, Source::Espanso, r, true);
                s.onboarding.completed.insert(path.clone());
            }
            if let Some(e) = err {
                let n = s.onboarding.completed.len();
                let total = s.onboarding.espanso.len();
                let prefix = if n > 0 { format!("Imported {n} of {total} folders; retry brings in the rest. ") } else { String::new() };
                fail(s, Source::Espanso, &e, &prefix);
            }
            s.onboarding.working = None;
        },
    );
}

fn run_import(s: &mut MainUi, source: Source, path: PathBuf) {
    begin(s, source);
    s.spawn(
        move || if source == Source::Espanso { commands::import_espanso(path) } else { commands::import_file(path) },
        move |s, result, _| {
            match result {
                Ok(r) => record(s, source, &r, false),
                Err(e) => fail(s, source, &e, ""),
            }
            s.onboarding.working = None;
        },
    );
}

fn choose_espanso_folder(s: &mut MainUi, tx: &mut Tx) {
    s.onboarding.picking = true;
    let options = FileDialogOptions { title: "Choose your Espanso config folder".into(), ..Default::default() };
    tx.open_folder(options, |s: &mut MainUi, result, _| {
        s.onboarding.picking = false;
        match result {
            Ok(Some(path)) => run_import(s, Source::Espanso, path),
            Ok(None) => {}
            Err(e) => fail(s, Source::Espanso, &e.to_string(), ""),
        }
    });
}

fn import_from_file(s: &mut MainUi, source: Source, tx: &mut Tx) {
    s.onboarding.picking = true;
    let options = match source {
        Source::Raycast => FileDialogOptions { title: "Choose your Raycast snippets export".into(), ..Default::default() }.filter("Raycast snippets", &["json"]),
        _ => FileDialogOptions { title: "Choose your TextExpander export".into(), ..Default::default() }.filter("TextExpander", &["textexpander", "csv"]),
    };
    tx.open_file(options, move |s: &mut MainUi, result, _| {
        s.onboarding.picking = false;
        match result {
            Ok(Some(path)) => run_import(s, source, path),
            Ok(None) => {}
            Err(e) => fail(s, source, &e.to_string(), ""),
        }
    });
}

// MARK: Pieces

fn h1(th: &Theme, s: &str) -> El {
    // `text-xl font-semibold tracking-tight`.
    text(s.to_string()).font(ty::XXL.weight_of(weight::SEMIBOLD).tracking(-0.5)).color(th.t.foreground).role("heading")
}

fn lead(th: &Theme, s: &str) -> El {
    text(s.to_string()).font(ty::SM).color(th.t.muted_foreground).wrap()
}

fn heading_block(th: &Theme, title: &str, body: El) -> El {
    col().gap(8.0).child(still(h1(th, title))).child(still(body))
}

/// `rounded-xl border border-hairline bg-surface/70 shadow-card`.
fn glass(th: &Theme, e: El) -> El {
    let t = &th.t;
    // `bg-surface/70` over the page. CSS doesn't paint a box-shadow under its own box; Rune does,
    // so a translucent fill would show shadow-card's drop shadow through it. Composite over
    // the background instead (the atmosphere's 7 % iris under it is lost to the mix).
    e.radius(radius::XL).fill(Theme::mix(t.surface, t.background, 0.7)).border(1.0, t.hairline).elevate(&t.elev_card)
}

fn spinner(color: Color, size: f32) -> El {
    icon(&icons::LOADER_CIRCLE, size).color(color).looping(LoopSpec::rotation(1.0)).a11y_hidden()
}

/// A `size="sm"` button whose leading icon is `<Loader2 className="animate-spin" />` while
/// `working` (the kit's Button has no spinning icon slot, so the label and spinner ride as one
/// child and the empty label's gap is taken back).
#[allow(clippy::too_many_arguments)]
fn action(th: &Theme, id: &str, label: &str, aria: String, variant: Variant, working: bool, idle_icon: Option<&'static rune::Icon>, disabled: bool, press: impl Fn(&mut MainUi, &mut Tx) + 'static) -> El {
    let t = &th.t;
    let ink = if variant == Variant::Default { t.primary_foreground } else { t.secondary_foreground };
    let b = if working {
        let font = TextStyle { size: 12.8, ..ty::SM.weight_of(weight::MEDIUM) };
        let inner = row().ml(-4.0).gap(4.0).center_y().child(spinner(ink, 14.0)).child(text(label.to_string()).font(font).color(ink)).a11y_hidden();
        Button::new("").child(inner)
    } else {
        let mut b = Button::new(label.to_string());
        if let Some(i) = idle_icon {
            b = b.icon(i);
        }
        b
    };
    whole(b.id(id.to_string()).variant(variant).size(Size::Sm).disabled(disabled).on_press_tx(press).build(th).key(id.to_string()).label(aria).min_w(ACTION_WIDTH))
}

fn source_row(th: &Theme, key: &str, name: &str, detail: String, outcome: Option<&Outcome>, error: Option<&String>, control: El) -> El {
    let t = &th.t;
    let head = row()
        .center_y()
        .justify_between()
        .gap(16.0)
        .child(
            whole(col())
                .min_w(0.0)
                .shrink()
                .gap(2.0)
                .child(text(name.to_string()).font(ty::SM.weight_of(weight::MEDIUM)).color(t.foreground))
                .child(text(detail).font(ty::XS).color(t.muted_foreground).wrap()),
        )
        .child(whole(rune::el().child(control)));
    let mut e = glass(th, col().key(format!("ob-src-{key}")).role("listitem").gap(8.0).px(16.0).py(12.0)).child(still(head));
    if let Some(o) = outcome {
        let c = if o.count > 0 { t.success } else { t.muted_foreground };
        let mut line = row().center_y().gap(6.0);
        if o.count > 0 {
            line = line.child(icon(&icons::CHECK, 14.0).color(c).key(format!("ob-ok-{key}")).transition(check_in()).a11y_hidden());
        }
        let msg = if o.count > 0 { format!("Imported {} snippet{}", o.count, plural(o.count)) } else { "No new snippets to add".into() };
        e = e.child(still(line.key("outcome").child(text(msg).key("message").font(ty::XS).color(c))));
        if o.changed > 0 {
            let verb = if o.changed == 1 { "needs" } else { "need" };
            e = e.child(still(
                text(format!("{} item{} {verb} a look — e.g. {}", o.changed, plural(o.changed), o.first_change.clone().unwrap_or_default()))
                    .key("changes").font(ty::XS)
                    .color(t.muted_foreground)
                    .wrap(),
            ));
        }
    }
    if let Some(err) = error {
        e = e.child(still(text(err.clone()).key("error").font(ty::XS).color(t.destructive).wrap().role("alert")));
    }
    e
}

// MARK: Steps

fn welcome(th: &Theme) -> El {
    let t = &th.t;
    let chip = text(";sig").font(ty::MONO_XS).color(t.iris_foreground).px(8.0).py(4.0).radius(radius::MD).fill(t.iris_soft);
    let lines = col()
        .child(text("Best regards,").font(ty::XS.lh(19.5)).color(t.muted_foreground))
        .child(text("Your Name").font(ty::XS.lh(19.5)).color(t.foreground));
    let card = glass(th, row().w_full().center_y().gap(12.0).px(16.0).py(12.0))
        .child(chip)
        .child(icon(&icons::ARROW_RIGHT, 14.0).color(t.faint).a11y_hidden())
        .child(lines);
    col()
        .key("ob-welcome")
        .items_center()
        .gap(24.0)
        .transition(rise())
        .child(still(wordmark(WordmarkVariant::Full, 224.0, t.foreground).mb(8.0)))
        .child(
            whole(col())
                .items_center()
                .gap(8.0)
                .child(text(TAGLINE).font(TextStyle::new(28.0, weight::SEMIBOLD).lh(20.0).tracking(-0.7)).color(t.foreground).align(Align::Center).role("heading"))
                .child(text(DESCRIPTION).font(ty::SM.lh(22.75)).color(t.muted_foreground).align(Align::Center).wrap()),
        )
        .child(whole(card))
        .child(whole(Button::new("Get started").size(Size::Lg).height(40.0).padding_x(20.0).icon_after(&icons::ARROW_RIGHT).on_press(next).build(th).key("ob-get-started")))
        .child(still(text("A few small steps, then back to your words.").font(ty::XS2).color(t.faint).mt(-12.0)))
}

fn permissions(s: &MainUi, th: &Theme) -> El {
    let ready = permissions_ready(&s.status);
    col()
        .key("ob-permissions")
        .gap(24.0)
        .transition(rise())
        .child(still(heading_block(th, "Let Kudzu type for you", lead(th, "Allow access once in macOS. We\u{2019}ll guide you through it and detect when you\u{2019}re ready."))))
        .child(whole(setup_card::<MainUi>(th, &s.status)))
        .child(
            still(row())
                .justify_between()
                .child(whole(Button::new("Skip for now").variant(Variant::Ghost).on_press(next).build(th).key("ob-perm-skip")))
                .child(whole(Button::new("Continue").icon_after(&icons::ARROW_RIGHT).disabled(!ready).on_press(next).build(th).key("ob-perm-continue"))),
        )
}

fn import(s: &MainUi, th: &Theme) -> El {
    let ob = &s.onboarding;
    let busy = ob.busy();
    let espanso_count: usize = ob.espanso.iter().map(|d| d.snippet_count).sum();
    let detail = match ob.detection {
        Detection::Looking => "Looking on this Mac…".to_string(),
        _ if !ob.espanso.is_empty() => format!("Found {espanso_count} snippets on this Mac."),
        Detection::Failed => "Couldn\u{2019}t check this Mac. Choose your Espanso folder instead.".to_string(),
        _ if ob.outcomes.contains_key(&Source::Espanso) => "From the folder you chose.".to_string(),
        _ => "Not found on this Mac. If you keep it elsewhere, choose its folder.".to_string(),
    };
    let failed = ob.failures.contains_key(&Source::Espanso);
    let done = ob.outcomes.contains_key(&Source::Espanso) && !failed;
    let working_esp = ob.working == Some(Source::Espanso);
    let espanso_btn = if !ob.espanso.is_empty() {
        let aria = if done { "Espanso imported".to_string() } else { format!("{} Espanso snippets", if failed { "Retry" } else { "Import" }) };
        let label = if done { "Imported" } else if failed { "Retry" } else { "Import" };
        action(th, "ob-espanso", label, aria, Variant::Default, working_esp, Some(&icons::DOWNLOAD), busy || done, |s, _| import_detected(s))
    } else {
        action(
            th,
            "ob-espanso-folder",
            "Choose folder…",
            "Choose folder… for Espanso".into(),
            Variant::Secondary,
            working_esp,
            None,
            busy || ob.detection == Detection::Looking,
            choose_espanso_folder,
        )
    };
    let mut list = still(col())
        .key("ob-sources")
        .role("list")
        .label("Import sources")
        .gap(8.0)
        .child(still(source_row(th, "espanso", "Espanso", detail, ob.outcomes.get(&Source::Espanso), ob.failures.get(&Source::Espanso), espanso_btn)));
    for (source, name, hint, key) in [(Source::Raycast, "Raycast", RAYCAST_HINT, "raycast"), (Source::TextExpander, "TextExpander", TEXTEXPANDER_HINT, "textexpander")] {
        let failed = ob.failures.contains_key(&source);
        let label = if failed { "Try again…" } else { "Choose file…" };
        let btn = action(th, &format!("ob-{key}"), label, format!("{label} for {name}"), Variant::Secondary, ob.working == Some(source), None, busy, move |s, tx| {
            import_from_file(s, source, tx)
        });
        list = list.child(still(source_row(th, key, name, hint.to_string(), ob.outcomes.get(&source), ob.failures.get(&source), btn)));
    }
    let footer = if ob.imported > 0 {
        Button::new("Continue").id("ob-import-continue").icon_after(&icons::ARROW_RIGHT).disabled(busy).on_press(next).build(th).key("ob-import-continue")
    } else {
        Button::new("Skip for now").id("ob-import-skip").variant(Variant::Ghost).disabled(busy).on_press(next).build(th).key("ob-import-skip")
    };
    let status = if ob.imported > 0 { format!("Imported {} snippet{}", ob.imported, plural(ob.imported)) } else { String::new() };
    col()
        .key("ob-import")
        .gap(24.0)
        .transition(rise())
        .child(still(heading_block(
            th,
            "Bring your snippets?",
            lead(th, "Switching from another expander? Forms, dates and clipboard variables come along; anything Kudzu can't run yet is kept as text. A backup is saved first."),
        )))
        .child(still(rune::el()).role("status").label(status).size(0.0, 0.0).absolute(0.0, 0.0))
        .child(list)
        .child(still(row()).justify_end().child(whole(footer)))
}

fn practice(s: &MainUi, th: &Theme) -> El {
    let t = &th.t;
    let ob = &s.onboarding;
    let success = ob.practice.text.contains(SAMPLE_TEXT);
    let finishing = ob.finishing;
    let intro = whole(row())
        .flex_wrap()
        .center_y()
        .child(text("Type ").font(ty::SM).color(t.muted_foreground))
        .child(text(SAMPLE_TRIGGER).font(TextStyle::new(14.0, weight::NORMAL).family(rune::Family::Mono).lh(20.0)).color(t.iris_foreground).px(4.0).radius(radius::SM).fill(t.iris_soft))
        .child(text(" below to see how expansion feels.").font(ty::SM).color(t.muted_foreground));
    let field = still(text_input(&ob.practice, ty::MD))
        .key("ob-practice-field")
        .label("Practice field")
        .w_full()
        .h(112.0)
        .padding(16.0)
        .color(t.foreground)
        .placeholder(format!("Type {SAMPLE_TRIGGER} here…"), t.faint)
        .caret(t.foreground, t.iris_soft)
        .disabled(finishing)
        .on_edit(|s: &mut MainUi| &mut s.onboarding.practice)
        .on_input_change(|s: &mut MainUi, _tx: &mut Tx| {
            // Kudzu never expands inside its own windows (so editing a snippet can't trigger one),
            // so this field shows the expansion itself.
            let v = s.onboarding.practice.text.clone();
            if v.to_lowercase().ends_with(SAMPLE_TRIGGER) {
                let cut = v.len() - SAMPLE_TRIGGER.len();
                s.onboarding.practice.replace_text(format!("{}{SAMPLE_TEXT}", &v[..cut]));
            }
        });
    let mut well = still(rune::el()).key("ob-practice-well").w_full().radius(radius::XL).fill(t.surface).elevate(&t.elev_card).motion(Spring::new(0.15, 0.0)).child(field);
    // `shadow-[0_0_0_1px_var(--success),0_0_24px_-6px_var(--success)]` is meant to replace
    // shadow-card on success, but on the web the custom `shadow-card` utility wins the cascade,
    // so the glow never shows (computed `box-shadow` stays shadow-card). Parity keeps it off;
    // flip `SUCCESS_GLOW` to draw the intended ring and glow.
    let glow_on = success && SUCCESS_GLOW;
    let ring = |a: f32| Shadow { spread: 1.0, color: t.success.alpha(t.success.a * a), ..Shadow::default() };
    let glow = |a: f32| Shadow { blur: 24.0, spread: -6.0, color: t.success.alpha(t.success.a * a), ..Shadow::default() };
    well = well.box_shadow(ring(if glow_on { 1.0 } else { 0.0 })).box_shadow(glow(if glow_on { 1.0 } else { 0.0 }));
    let tail = if success {
        let star = still(rune::el()).key("ob-star").child(still(icon(&icons::SPARKLES, 16.0).color(t.success))).sequence(Sequence::new(STAR)).a11y_hidden();
        col()
            .key("ob-success")
            .gap(16.0)
            .transition(rise())
            .child(still(row()).center_y().gap(8.0).child(star).child(still(text("That's how a snippet expands.").font(ty::SM.weight_of(weight::MEDIUM)).color(t.success))))
            .child(still(
                text("When expansion is enabled, your library\u{2019}s triggers work in Notes, Mail, your browser, and other apps.")
                    .font(ty::XS)
                    .color(t.muted_foreground)
                    .wrap(),
            ))
            .child(
                still(row())
                    .flex_wrap()
                    .center_y()
                    .gap(6.0)
                    .child(still(text("Forgot a trigger? Press").font(ty::XS).color(t.muted_foreground)))
                    .child(whole(Shortcut::keys(accelerator_labels(&s.settings.palette_hotkey)).size(ShortcutSize::Md).build(th)))
                    .child(still(text("anywhere to search your snippets.").font(ty::XS).color(t.muted_foreground))),
            )
            .child(
                still(row()).justify_end().child(whole(
                    Button::new(if finishing { "Opening…" } else { "Open library" })
                        .id("ob-open-library")
                        .icon_after(&icons::ARROW_RIGHT)
                        .disabled(finishing)
                        .on_press(next)
                        .build(th)
                        .key("ob-open-library"),
                )),
            )
    } else {
        still(row()).key("ob-skip-row").justify_end().child(whole(Button::new("Skip").id("ob-practice-skip").variant(Variant::Ghost).disabled(finishing).on_press(next).build(th).key("ob-practice-skip")))
    };
    col()
        .key("ob-practice")
        .gap(24.0)
        .transition(rise())
        .child(still(heading_block(th, "Try a preview", intro)))
        .child(well)
        .child(tail)
}

fn progress(s: &MainUi, th: &Theme) -> El {
    let t = &th.t;
    let index = s.onboarding.index();
    let items = STEPS.iter().enumerate().map(|(i, step)| {
        let current = i == index;
        let mark = if i < index {
            icon(&icons::CHECK, 12.0).color(t.iris).key(format!("ob-done-{i}")).transition(check_in()).a11y_hidden()
        } else {
            still(rune::el()).size(6.0, 6.0).radius(radius::FULL).fill(if current { t.iris } else { Theme::tint(t.foreground, 0.15) }).a11y_hidden()
        };
        still(row())
            .key(format!("ob-step-{i}"))
            .center_y()
            .gap(6.0)
            .role("listitem")
            .child(mark)
            .child(still(text(step.label()).key("label")).font(ty::XXS).color(if current { t.iris_foreground } else { t.faint }).motion(Spring::new(0.15, 0.0)))
    });
    still(row()).key("ob-progress").role("list").label("Setup progress").justify_center().gap(20.0).children(items)
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    detect(s);
    let th = theme_for(cx);
    let t = &th.t;
    let ob = &s.onboarding;
    let [w, h] = cx.layout_size("onboarding").unwrap_or([1180.0, 760.0]);
    let index = ob.index();

    let step = match ob.step {
        Step::Welcome => welcome(th),
        Step::Permissions => permissions(s, th),
        Step::Import => import(s, th),
        Step::Practice => practice(s, th),
    };
    let mut region = col()
        .key(format!("ob-region-{:?}", ob.step))
        .transition(Transition::none())
        .role("region")
        .label(format!("Setup: {}", ob.step.label()))
        .w_full()
        .max_w(480.0)
        .gap(32.0)
        .px(24.0)
        .py(96.0)
        .child(step);
    if let Some(e) = &ob.error {
        region = region.child(whole(text(e.clone()).key("error")).font(ty::XS).color(t.destructive).wrap().padding(12.0).radius(radius::LG).fill(Theme::tint(t.destructive, 0.1)).role("alert"));
    }
    region = region.child(progress(s, th));

    // `my-auto` inside the scrolling column: centred while it fits.
    let page = still(col()).w_full().min_h(h).items_center().justify_center().child(region);
    let scroll = still(col()).key("ob-scroll").w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page);

    // `onboarding-atmosphere`: `radial-gradient(ellipse at 50% 30%, iris 7%, transparent 60%)`,
    // an ellipse through the farthest corner. Chrome gives it farthest-side proportions (measured
    // against the web build: the visible edge sits ~960 × 907 px from the centre at 1180 × 760),
    // so the radii are the farthest sides scaled by √2.
    let k = std::f32::consts::SQRT_2;
    let atmosphere = still(rune::el())
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .radial_gradient([0.5, 0.3], [0.5 * w * k, 0.7 * h * k], [(0.0, Theme::tint(t.iris, 0.07)), (0.6, Theme::tint(t.iris, 0.0))])
        .inert()
        .a11y_hidden();

    let mut header = still(row()).pin(Some(0.0), Some(56.0), Some(0.0), None).px(32.0).center_y().justify_between();
    // `invisible` on welcome: it shows and hides at once.
    let mark = wordmark(WordmarkVariant::Small, 80.0, t.muted_foreground).opacity_motion(Spring::NONE);
    header = header.child(still(if ob.step == Step::Welcome { mark.opacity(0.0).a11y_hidden() } else { mark }));
    if index > 0 {
        let back = STEPS[index - 1];
        header = header.child(Button::new("Back").id("ob-back").variant(Variant::Ghost).size(Size::Sm).icon(&icons::ARROW_LEFT).disabled(ob.busy()).on_press(move |s: &mut MainUi| go(s, back)).build(th).key("ob-back").transition(Transition::none()).instant_changes());
    }
    let drag = still(rune::el()).pin(Some(0.0), Some(0.0), Some(0.0), None).h(48.0).window_drag();

    rune::el()
        .key("onboarding")
        .transition(Transition::none())
        .w_full()
        .h_full()
        .fill(t.background)
        .clip_rounded(0.0)
        .child(atmosphere)
        .child(scroll)
        .child(drag)
        .child(header)
}
