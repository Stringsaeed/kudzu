//! `Agents` in settings.tsx: AI agent access over MCP, what agents never see, and how to connect
//! one (the `claude mcp add` command and JSON to copy, and the Claude Code skill).

use rune::{col, icon, rich, row, text, Cx, El, Span};
use serde_json::json;

use super::{desc, group, page, row_el, spinner};
use crate::backend::agent;
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::switch::Switch;
use crate::ui::kit::toast::Toast;
use crate::ui::main::insights::{still, whole};
use crate::ui::main::{MainUi, SettingsSection};
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

/// `useEffect(() => void reload(), [on])`: loads the setup when the section mounts and whenever
/// **Allow AI agents** changes.
pub(super) fn reload_if_needed(s: &MainUi, section: SettingsSection) {
    if section != SettingsSection::Agents {
        return;
    }
    let on = s.settings.agent_access;
    if s.settings_page.agent_for.get() == Some(on) {
        return;
    }
    s.settings_page.agent_for.set(Some(on));
    reload(s);
}

fn reload(s: &MainUi) {
    s.spawn(agent::agent_setup, |s, setup, _| page(s).agent = Some(setup));
}

/// `installSkill`.
fn install_skill(s: &mut MainUi) {
    page(s).installing = true;
    s.spawn(agent::install_agent_skill, |s, result, _| {
        match result {
            Ok(path) => s.toasts.push(Toast::success("Skill installed").description(path)),
            Err(e) => s.toasts.push(Toast::error("Couldn't install the skill").description(e)),
        }
        page(s).installing = false;
        reload(s);
    });
}

/// `copyText`.
fn copy(s: &mut MainUi, tx: &mut rune::Tx, value: &str, what: &str) {
    tx.copy(value.to_string());
    s.toasts.push(Toast::success(format!("Copied {what}")));
}

/// `<CopyBlock label value>`: a mono block in a well, scrolling sideways, with a copy button.
fn copy_block(th: &Theme, label: &'static str, value: String) -> El {
    let t = &th.t;
    let fill = if th.dark { rune::Color::BLACK.alpha(0.15) } else { Theme::tint(t.foreground, 0.025) };
    let font = rune::TextStyle::new(11.5, weight::NORMAL).family(rune::Family::Mono).lh(11.5 * 1.625);
    let pre = still(row().key(format!("copy-{label}-scroll")).grow(1.0).min_w(0.0).scroll_x())
        .child(text(value.clone()).font(font).color(t.foreground).selectable(true).py(10.0).pl(12.0));
    let button = Button::icon_only(&icons::COPY, format!("Copy {label}"))
        .variant(Variant::Ghost)
        .size(Size::IconXs)
        .on_press_tx(move |s: &mut MainUi, tx| copy(s, tx, &value, label))
        .build(th);
    whole(row())
        .mt(8.0)
        .items_start()
        .gap(4.0)
        .radius(radius::LG)
        .fill(fill)
        .elevate(&t.elev_well)
        .child(pre)
        .child(still(rune::el()).mt(6.0).mr(6.0).child(button))
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> Vec<El> {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.settings;
    let setup = s.settings_page.agent.as_ref();
    let on = st.agent_access;
    let access_desc = if on {
        if setup.is_some_and(|x| x.running) { "Agents you've connected can use Kudzu while it's running." } else { "Couldn't start. Turn it off and on again, or restart Kudzu." }
    } else {
        "Off. Connected agents are told Kudzu is unavailable."
    };
    let access = group(
        th,
        "AI agents",
        Some("Let assistants like Claude Code look up your snippets, expand them and save new ones, using the Model Context Protocol (MCP). Agents connect to Kudzu on this Mac; Kudzu doesn't upload anything."),
        vec![
            row_el(
                th,
                "Allow AI agents",
                Some(desc(th, access_desc)),
                vec![Switch::new(on)
                    .label("Allow AI agents")
                    .id("agent-on")
                    .on_toggle(|s: &mut MainUi| {
                        let on = !s.settings.agent_access;
                        s.update_settings(json!({ "agentAccess": on }))
                    })
                    .build(th)],
            ),
            row_el(
                th,
                "Create and edit snippets",
                Some(desc(th, "Agents can add snippets and change existing ones. They can't delete anything.")),
                vec![Switch::new(st.agent_write)
                    .label("Create and edit snippets")
                    .id("agent-write")
                    .disabled(!on)
                    .on_toggle(|s: &mut MainUi| {
                        let v = !s.settings.agent_write;
                        s.update_settings(json!({ "agentWrite": v }))
                    })
                    .build(th)],
            ),
            row_el(
                th,
                "Share clipboard history",
                Some(desc(th, if st.clipboard_enabled { "Agents can search what you've copied, such as a link or an error message." } else { "Clipboard history is turned off." })),
                vec![Switch::new(st.agent_clipboard && st.clipboard_enabled)
                    .label("Share clipboard history")
                    .id("agent-clip")
                    .disabled(!on || !st.clipboard_enabled)
                    .on_toggle(|s: &mut MainUi| {
                        let v = !(s.settings.agent_clipboard && s.settings.clipboard_enabled);
                        s.update_settings(json!({ "agentClipboard": v }))
                    })
                    .build(th)],
            ),
        ],
    );

    // `flex gap-3 py-3.5` with a ShieldCheck beside two paragraphs.
    let p1_lead = "Secrets are withheld from agents.";
    let p1 = format!(
        "{p1_lead} Before anything reaches an agent, Kudzu replaces text that looks like a password, API key, access token or private key with [redacted]. Clipboard items that are only a secret aren't shared at all, and agents can't search for them."
    );
    let r = p1.find("[redacted]").unwrap_or(0);
    let p1 = rich(p1, vec![Span::new(0, p1_lead.len()).weight(weight::MEDIUM).color(t.foreground), Span::new(r, r + "[redacted]".len()).mono()])
        .font(ty::XS)
        .color(t.muted_foreground)
        .wrap();
    let p2 = desc(th, "Detection is a best guess. A short, plain password can look like an ordinary word, so keep passwords in a password manager; Kudzu never records copies from one. In clipboard history, you can mark an item as a secret, or as not one, and agents follow your choice.");
    let secrets = group(
        th,
        "Passwords and keys",
        None,
        vec![whole(row())
            .gap(12.0)
            .py(14.0)
            .child(icon(&icons::SHIELD_CHECK, 16.0).color(t.success).mt(2.0).a11y_hidden())
            .child(col().grow(1.0).min_w(0.0).gap(6.0).child(p1).child(p2))],
    );

    let title = |s: &str| text(s.to_string()).font(ty::BASE.weight_of(weight::MEDIUM)).color(t.foreground);
    let mut claude = whole(col()).py(14.0).child(title("Claude Code")).child(desc(th, "Run this in Terminal:"));
    if let Some(x) = setup {
        claude = claude.child(copy_block(th, "command", x.claude_command.clone()));
    }
    let skill_desc = match setup.map(|x| x.skill) {
        Some("current") => {
            let home = setup.map(|x| tildify(&x.skill_path)).unwrap_or_default();
            format!("Installed in {home}")
        }
        Some("outdated") => "This version of Kudzu has a newer skill.".to_string(),
        _ => "Teaches Claude when to use your snippets and how to write Kudzu templates.".to_string(),
    };
    let skill_control = if setup.is_some_and(|x| x.skill == "current") {
        row()
            .center_y()
            .gap(6.0)
            .child(icon(&icons::CHECK, 14.0).color(t.success).a11y_hidden())
            .child(text("Installed").font(ty::XS).color(t.muted_foreground))
    } else {
        let label = if setup.is_some_and(|x| x.skill == "outdated") { "Update skill" } else { "Install skill" };
        let installing = s.settings_page.installing;
        let mut b = Button::new(if installing { "" } else { label }).size(Size::Sm).variant(Variant::Secondary).disabled(installing || setup.is_none());
        if installing {
            let font = rune::TextStyle { size: 12.8, ..ty::SM.weight_of(weight::MEDIUM) };
            b = b.child(row().ml(-4.0).gap(4.0).center_y().child(spinner(14.0, t.secondary_foreground)).child(text(label).font(font).color(t.secondary_foreground)).a11y_hidden());
        }
        b.on_press(install_skill).build(th).label(label)
    };
    let mut other = whole(col())
        .py(14.0)
        .child(title("Other apps"))
        .child(desc(th, "Claude Desktop, Cursor and other MCP apps: add this to their MCP server settings."));
    if let Some(x) = setup {
        other = other.child(copy_block(th, "configuration", x.config_json.clone()));
        if let Some(dir) = &x.bundled_skill_dir {
            let a = "For other agents, the skill is also in ";
            let b = ", and connected agents can read it as ";
            let uri = "kudzu://skill/SKILL.md";
            let body = format!("{a}{dir}{b}{uri}.");
            let d0 = a.len();
            let d1 = d0 + dir.len();
            let u0 = d1 + b.len();
            other = other.child(
                rich(body, vec![Span::new(d0, d1).mono().color(Theme::tint(t.foreground, 0.85)), Span::new(u0, u0 + uri.len()).mono()])
                    .font(ty::XS)
                    .color(t.muted_foreground)
                    .selectable(true)
                    .wrap()
                    .mt(8.0),
            );
        }
    }
    let connect = group(
        th,
        "Connect an agent",
        Some("Do this once per app. The agent starts Kudzu's MCP server itself; Kudzu just needs to be running."),
        vec![claude, row_el(th, "Claude Code skill", Some(desc(th, skill_desc)), vec![skill_control]), other],
    );
    vec![access, secrets, connect]
}

/// `path.replace(/^\/Users\/[^/]+/, "~")`.
fn tildify(path: &str) -> String {
    match path.strip_prefix("/Users/").and_then(|rest| rest.find('/').map(|i| &rest[i..])) {
        Some(rest) => format!("~{rest}"),
        None => path.to_string(),
    }
}
