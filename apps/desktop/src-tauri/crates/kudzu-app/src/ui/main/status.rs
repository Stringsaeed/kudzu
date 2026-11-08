//! The sidebar's footer (`components/app/main/status.tsx`): whether Kudzu is expanding, with a
//! tooltip that explains it (and the quick-search hotkey), and the pause button.

use rune::{col, el, icon, row, text, Cx, El, Shadow, Side};

use super::{MainUi, SettingsSection, View};
use crate::ui::icons;
use crate::ui::kit::button::{hover_fast, layer};
use crate::ui::kit::shortcut::{accelerator_labels, Shortcut, Tone};
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;
use kudzu_engine::{EngineStatus, TapMode};

/// `Health` in status.tsx.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Ready,
    Paused,
    Limited,
    Blocked,
    Secure,
}

/// `health(status, platform)` on macOS.
pub fn health(status: &EngineStatus) -> Health {
    if status.paused {
        return Health::Paused;
    }
    if status.secure_input {
        return Health::Secure;
    }
    let p = &status.permissions;
    if status.tap == TapMode::Off || (!p.input_monitoring && !p.accessibility) {
        return Health::Blocked;
    }
    if !p.accessibility || status.tap == TapMode::ListenOnly {
        return Health::Limited;
    }
    Health::Ready
}

/// `COPY[health]`: label and detail.
pub fn copy(h: Health) -> (&'static str, &'static str) {
    match h {
        Health::Ready => ("Expanding", "Kudzu is listening for your triggers in every app."),
        Health::Paused => ("Paused", "Nothing expands until you resume."),
        Health::Limited => ("Limited", "Grant Accessibility so Kudzu can type expansions reliably."),
        Health::Blocked => ("Needs permission", "Kudzu can't see your typing yet. Open Settings → Permissions."),
        Health::Secure => ("Secure Input on", "A password field or app has Secure Input on, so macOS blocks expansion."),
    }
}

/// `<StatusIndicator />`.
pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer_out: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let h = health(&s.status);
    let (label, base) = copy(h);
    let detail = match (&s.status.secure_input_owner, h) {
        (Some(owner), Health::Secure) => format!("{owner} has Secure Input on, so macOS blocks expansion."),
        _ => base.to_string(),
    };

    // `size-1.5 rounded-full`: success glows (`shadow-[0_0_8px_var(--success)]`).
    let mut dot = el().size(6.0, 6.0).radius(radius::FULL).a11y_hidden();
    dot = match h {
        Health::Ready => dot.fill(t.success).box_shadow(Shadow { x: 0.0, y: 0.0, blur: 8.0, spread: 0.0, color: t.success, inset: false }),
        Health::Blocked => dot.fill(t.destructive),
        _ => dot.fill(t.warning),
    };
    let actionable = matches!(h, Health::Blocked | Health::Limited);
    let mut status = hover_fast(row())
        .key("status-button")
        .role("button")
        .label(format!("Status: {label}. {detail}"))
        .w_full()
        .min_w(0.0)
        .gap(8.0)
        .px(8.0)
        .py(4.0)
        .center_y()
        .radius(radius::MD)
        .child(layer(0.0).radius(radius::MD).fill(t.sidebar_accent).group_opacity(0.0, 1.0))
        .child(dot)
        .child(text(label).font(ty::XS.weight_of(weight::MEDIUM)).color(Theme::tint(t.sidebar_foreground, 0.8)).truncate().shrink().a11y_hidden())
        .focusable(true)
        .focus_ring(2.0, 0.0, t.ring)
        .on_press(move |s: &mut MainUi| {
            if actionable {
                s.set_view(View::Settings(Some(SettingsSection::General)));
            }
        });
    if matches!(h, Health::Blocked | Health::Secure) {
        status = status.child(icon(&icons::SHIELD_ALERT, 14.0).color(t.warning).a11y_hidden());
    }
    // `<p>{detail}</p><p className="mt-1 flex items-center gap-1 opacity-70">Quick search anywhere: <Shortcut/></p>`.
    let tip = col()
        .shrink()
        .min_w(0.0)
        .child(text(detail.clone()).font(ty::XS).color(t.background).wrap())
        .child(
            row()
                .mt(4.0)
                .gap(4.0)
                .center_y()
                .opacity(0.7)
                .child(text("Quick search anywhere:").font(ty::XS).color(t.background))
                .child(Shortcut::keys(accelerator_labels(&s.settings.palette_hotkey)).tone(Tone::Tooltip).build(th)),
        );
    let status = Tooltip::new("status-tip").side(Side::Top).max_width(256.0).child(tip).build(cx, status, layer_out);

    let paused = s.status.paused;
    let pause = hover_fast(row())
        .key("status-pause")
        .role("button")
        .label(if paused { "Resume expansion" } else { "Pause expansion" })
        .checked(paused)
        .size(28.0, 28.0)
        .center()
        .radius(radius::MD)
        .child(layer(0.0).radius(radius::MD).fill(t.sidebar_accent).group_opacity(0.0, 1.0))
        .child(icon(if paused { &icons::PLAY } else { &icons::PAUSE }, 14.0).color(t.muted_foreground).group_color(t.foreground).a11y_hidden())
        .press_sink(0.97)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.ring)
        .on_press(move |s: &mut MainUi| s.set_paused(!paused));
    let pause = Tooltip::new("pause-tip").side(Side::Top).text(if paused { "Resume" } else { "Pause" }).shortcut("mod+shift+p").build(cx, pause, layer_out);

    row()
        .key("status")
        .w_full()
        .gap(4.0)
        .px(10.0)
        // `border-t py-2`: the 1 pt border takes room on the web.
        .pt(9.0)
        .pb(8.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
        .child(status.grow(1.0).min_w(0.0))
        .child(pause)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_reads_like_status_tsx() {
        let mut st = EngineStatus {
            can_post_events: true,
            can_expand: true,
            paused: false,
            tap: TapMode::Active,
            permissions: kudzu_engine::PermissionStatus { accessibility: true, input_monitoring: true },
            secure_input: false,
            secure_input_owner: None,
            summary: String::new(),
        };
        assert_eq!(health(&st), Health::Ready);
        st.tap = TapMode::ListenOnly;
        assert_eq!(health(&st), Health::Limited);
        st.tap = TapMode::Off;
        assert_eq!(health(&st), Health::Blocked);
        st.secure_input = true;
        assert_eq!(health(&st), Health::Secure);
        st.paused = true;
        assert_eq!(health(&st), Health::Paused);
    }
}
