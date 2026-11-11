mod doc;
mod field;
mod labels;
mod proof;

use rune::*;

use field::State;

fn main() {
    codec_holds();
    if rune::is_headless() {
        rune::run(spec());
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&proof::to_json(&proof::exercise())).unwrap()
        );
    }
}

/// Checks the codec before a snapshot or a proof run.
fn codec_holds() {
    let source = "Hello {{date}} there";
    let doc = doc::from_template(source, None);
    assert_eq!(doc.to_template().0, source);
    assert_eq!(labels::date_presets().len(), 11);
    let parsed = kudzu_core::template::parse("{{date}}");
    let kudzu_core::template::Node::Tag(tag) = &parsed.nodes[0] else {
        panic!("date tag");
    };
    assert_eq!(
        labels::chip_face(tag, "{{date}}").0,
        "Date \u{00b7} ISO date"
    );
}

fn spec() -> AppSpec<State> {
    AppSpec {
        scenes: &[
            ("chips", |_| {}),
            ("formatted", |s| s.formatted = true),
            ("slash", |s| {
                s.edit = TextEdit::new("Say /").rich();
                s.edit.set_caret(s.edit.text.len());
            }),
            ("misspelled", |s| {
                s.edit = TextEdit::new("This is mispeled today").rich();
                s.spellcheck = true;
            }),
        ],
        families: ("System Font, Figtree", "Paper Mono, ui-monospace"),
        ..AppSpec::new("S13 editor", (720.0, 280.0), State::default, field::view)
    }
}
