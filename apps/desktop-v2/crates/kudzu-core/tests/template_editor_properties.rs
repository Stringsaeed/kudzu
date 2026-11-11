#[path = "template_support/editor.rs"]
mod editor;
mod template_support;
#[test]
fn seeded_display_source_and_untouched_marks_survive_edit_sequences() {
    editor::exercise(0x5323_B1B2_2026, 4096);
}
