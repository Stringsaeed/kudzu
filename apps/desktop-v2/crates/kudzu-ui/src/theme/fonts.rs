//! The bundled faces and the families the sans and mono slots name.
//!
//! The UI face is SF Pro ("System Font", which Rune loads from the system), with Figtree as the
//! bundled fallback; mono is Paper Mono, then the system monospace (`globals.css:174`).
//! Rune reads TTF and OTF, not WOFF2: `PaperMono-Variable.ttf` was converted once from the WOFF2
//! S01 copied, with `woff2_decompress` (the commit that added it records the command).
//!
//! `KUDZU_FONTS=bundled` drops the system face, so goldens render the same on every Mac.

/// Figtree, variable weight (OFL; `assets/fonts/Figtree-OFL.txt`).
pub static FIGTREE: &[u8] = include_bytes!("../../../../assets/fonts/Figtree[wght].ttf");
/// Paper Mono, variable weight 100–900 (OFL; `assets/fonts/PaperMono-OFL.txt`).
pub static PAPER_MONO: &[u8] = include_bytes!("../../../../assets/fonts/PaperMono-Variable.ttf");

/// The files every window registers (`AppSpec::fonts`).
pub static FILES: &[&[u8]] = &[FIGTREE, PAPER_MONO];

/// `(sans, mono)` as CSS family lists, the way the Tauri app writes them.
pub const SYSTEM: (&str, &str) = ("System Font, Figtree", "Paper Mono, ui-monospace");
/// The same with only bundled faces: for goldens (`KUDZU_FONTS=bundled`).
pub const BUNDLED: (&str, &str) = ("Figtree", "Paper Mono");

/// The families for this process: [`BUNDLED`] when `KUDZU_FONTS=bundled`, else [`SYSTEM`].
pub fn families() -> (&'static str, &'static str) {
    choose(std::env::var("KUDZU_FONTS").ok().as_deref())
}

fn choose(setting: Option<&str>) -> (&'static str, &'static str) {
    if setting == Some("bundled") {
        BUNDLED
    } else {
        SYSTEM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `name` table's family (ID 1) or typographic family (ID 16), whichever is present.
    fn family_names(font: &[u8]) -> Vec<String> {
        let be16 = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]) as usize;
        let tables = be16(4);
        let name = (0..tables)
            .map(|i| 12 + i * 16)
            .find(|&rec| &font[rec..rec + 4] == b"name")
            .map(|rec| u32::from_be_bytes(font[rec + 8..rec + 12].try_into().unwrap()) as usize)
            .expect("a name table");
        let (count, strings) = (be16(name + 2), name + be16(name + 4));
        let mut names = Vec::new();
        for i in 0..count {
            let rec = name + 6 + i * 12;
            let (platform, id, len, off) =
                (be16(rec), be16(rec + 6), be16(rec + 8), be16(rec + 10));
            if platform == 3 && (id == 1 || id == 16) {
                let utf16: Vec<u16> = font[strings + off..strings + off + len]
                    .chunks(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                names.push(String::from_utf16_lossy(&utf16));
            }
        }
        names
    }

    #[test]
    fn bundled_files_are_truetype_with_the_named_families() {
        for (font, family) in [(FIGTREE, "Figtree"), (PAPER_MONO, "Paper Mono")] {
            assert_eq!(&font[..4], &[0, 1, 0, 0], "{family} is TrueType");
            assert!(
                family_names(font).iter().any(|n| n == family),
                "{family}: {:?}",
                family_names(font)
            );
        }
    }

    #[test]
    fn bundled_setting_drops_the_system_face() {
        assert_eq!(choose(Some("bundled")), BUNDLED);
        assert_eq!(choose(None), SYSTEM);
        assert_eq!(choose(Some("system")), SYSTEM);
        assert!(!BUNDLED.0.contains("System Font"));
    }
}
