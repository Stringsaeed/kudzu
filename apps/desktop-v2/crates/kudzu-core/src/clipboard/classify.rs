//! What was copied: files, text (plain, code, link, colour), an image, or something else,
//! plus the content fingerprint that folds repeats together.

use super::{uti, ClipKind, PasteItem};

/// A copy, understood. Produced by [`classify`] from raw pasteboard items.
#[derive(Debug, Clone, PartialEq)]
pub struct Classified {
    pub kind: ClipKind,
    /// Full plain text.
    pub text: Option<String>,
    pub files: Vec<String>,
    pub color: Option<String>,
    pub rich: bool,
    /// The preferred image type present, if any.
    pub image_type: Option<String>,
    pub types: Vec<String>,
    pub size: u64,
    pub hash: u64,
}

/// Whether the source marked this copy as private or transient (see [`uti::PRIVATE_MARKERS`]).
pub fn is_private(items: &[PasteItem]) -> bool {
    items
        .iter()
        .flatten()
        .any(|(t, _)| uti::PRIVATE_MARKERS.contains(&t.as_str()))
}

pub(super) fn find<'a>(items: &'a [PasteItem], kind: &str) -> Option<&'a [u8]> {
    items
        .iter()
        .flatten()
        .find(|(t, _)| t == kind)
        .map(|(_, d)| d.as_slice())
}

/// Bundle ids of apps whose copies Kudzu never records by default (password managers).
pub fn default_ignored_apps() -> Vec<String> {
    [
        "com.1password.1password",
        "com.agilebits.onepassword7",
        "com.agilebits.onepassword-osx",
        "com.bitwarden.desktop",
        "com.apple.keychainaccess",
        "com.apple.Passwords",
        "org.keepassxc.keepassxc",
        "com.dashlane.dashlanephonefinal",
        "com.lastpass.LastPass",
        "in.sinew.Enpass-Desktop",
        "com.nordsec.nordpass",
        "me.proton.pass.electron",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Decides what was copied. `None` when there's nothing worth keeping (empty, or only whitespace).
pub fn classify(items: &[PasteItem], source_bundle: Option<&str>) -> Option<Classified> {
    let types: Vec<String> = items
        .first()
        .map(|i| i.iter().map(|(t, _)| t.clone()).collect())
        .unwrap_or_default();
    let size: u64 = items.iter().flatten().map(|(_, d)| d.len() as u64).sum();
    let all_types = || items.iter().flatten().map(|(t, _)| t.as_str());
    let rich = all_types().any(|t| {
        t == uti::RTF || t == uti::HTML || t == "public.rtfd" || t == "com.apple.flat-rtfd"
    });

    // Files: one `public.file-url` per item.
    let files: Vec<String> = items
        .iter()
        .filter_map(|item| item.iter().find(|(t, _)| t == uti::FILE_URL))
        .filter_map(|(_, d)| std::str::from_utf8(d).ok())
        .filter_map(file_url_to_path)
        .collect();
    if !files.is_empty() {
        let hash = fnv1a([b"f".as_slice(), files.join("\n").as_bytes()]);
        let text = Some(files.join("\n"));
        return Some(Classified {
            kind: ClipKind::File,
            text,
            files,
            color: None,
            rich: false,
            image_type: None,
            types,
            size,
            hash,
        });
    }

    let text = find(items, uti::PLAIN_TEXT)
        .or_else(|| find(items, uti::LEGACY_TEXT))
        .and_then(|d| String::from_utf8(d.to_vec()).ok())
        .filter(|s| !s.trim().is_empty());
    let image_type = uti::IMAGES
        .iter()
        .map(|(t, _)| *t)
        .find(|t| all_types().any(|x| x == *t))
        .map(String::from);

    // Text wins over an image when both are present (spreadsheets put a picture of the cells
    // next to their text); a lone image is an image.
    if let Some(text) = text {
        let trimmed = text.trim();
        let (kind, color) = if let Some(c) = parse_color(trimmed) {
            (ClipKind::Color, Some(c))
        } else if is_link(trimmed)
            || (find(items, uti::URL).is_some() && !trimmed.contains(char::is_whitespace))
        {
            (ClipKind::Link, None)
        } else if looks_like_code(&text, source_bundle) {
            (ClipKind::Code, None)
        } else {
            (ClipKind::Text, None)
        };
        let hash = fnv1a([b"t".as_slice(), text.as_bytes()]);
        return Some(Classified {
            kind,
            text: Some(text),
            files: vec![],
            color,
            rich,
            image_type: None,
            types,
            size,
            hash,
        });
    }

    if let Some(image) = &image_type {
        let data = find(items, image).unwrap_or_default();
        if !data.is_empty() {
            let hash = fnv1a([b"i".as_slice(), data]);
            return Some(Classified {
                kind: ClipKind::Image,
                text: None,
                files: vec![],
                color: None,
                rich: false,
                image_type: image_type.clone(),
                types,
                size,
                hash,
            });
        }
    }

    // Nothing at all, or only blank text.
    let textual = |t: &str| {
        t == uti::PLAIN_TEXT
            || t == uti::LEGACY_TEXT
            || t.starts_with("public.utf16")
            || t == "public.text"
    };
    if size == 0 || all_types().all(textual) {
        return None;
    }
    let mut parts: Vec<&[u8]> = vec![b"o"];
    for (t, d) in items.iter().flatten() {
        parts.push(t.as_bytes());
        parts.push(d);
    }
    Some(Classified {
        kind: ClipKind::Other,
        text: None,
        files: vec![],
        color: None,
        rich,
        image_type: None,
        types,
        size,
        hash: fnv1a(parts),
    })
}

/// 64-bit FNV-1a over several byte slices (stable across runs and platforms).
pub fn fnv1a<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // Separator, so ("ab","c") and ("a","bc") differ.
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// `file:///Users/me/My%20File.txt` → `/Users/me/My File.txt`.
pub fn file_url_to_path(url: &str) -> Option<String> {
    let rest = url.trim().trim_end_matches('\0').strip_prefix("file://")?;
    // Optional host ("localhost") before the path.
    let path = if rest.starts_with('/') {
        rest
    } else {
        &rest[rest.find('/')?..]
    };
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let mut s = String::from_utf8(out).ok()?;
    if s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    Some(s)
}

/// A single web, mail or FTP address.
pub fn is_link(text: &str) -> bool {
    if text.is_empty() || text.len() > 4096 || text.contains(char::is_whitespace) {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    let rest = ["https://", "http://", "ftp://", "mailto:"]
        .iter()
        .find_map(|p| lower.strip_prefix(p));
    match rest {
        Some(r) => {
            !r.is_empty()
                && (lower.starts_with("mailto:") || r.contains('.') || r.starts_with("localhost"))
        }
        None => lower.starts_with("www.") && lower[4..].contains('.'),
    }
}

/// A CSS colour (`#0af`, `#0a0bfc80`, `rgb(…)`, `hsl(…)`, `oklch(…)` …), normalized for display.
pub fn parse_color(text: &str) -> Option<String> {
    let t = text.trim();
    if t.len() > 64 {
        return None;
    }
    if let Some(hex) = t.strip_prefix('#') {
        return (matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| format!("#{}", hex.to_ascii_lowercase()));
    }
    let lower = t.to_ascii_lowercase();
    let open = lower.find('(')?;
    let name = &lower[..open];
    if ![
        "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch",
    ]
    .contains(&name)
        || !lower.ends_with(')')
    {
        return None;
    }
    let args = &lower[open + 1..lower.len() - 1];
    let numbers = args
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .count();
    let allowed = args
        .chars()
        .all(|c| c.is_ascii_digit() || " .,%/-+edgrantu".contains(c));
    (allowed && (3..=4).contains(&numbers) && args.chars().any(|c| c.is_ascii_digit()))
        .then(|| lower.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Apps whose text is almost always code.
const CODE_APPS: [&str; 16] = [
    "com.microsoft.VSCode",
    "com.microsoft.VSCodeInsiders",
    "com.todesktop.230313mzl4w4u92", // Cursor
    "dev.zed.Zed",
    "com.apple.dt.Xcode",
    "com.sublimetext.4",
    "com.panic.Nova",
    "com.jetbrains.intellij",
    "com.jetbrains.WebStorm",
    "com.jetbrains.pycharm",
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "com.mitchellh.ghostty",
    "net.kovidgoyal.kitty",
    "com.github.wez.wezterm",
];

const COMMANDS: [&str; 24] = [
    "git ", "npm ", "pnpm ", "yarn ", "npx ", "cargo ", "brew ", "cd ", "ls ", "curl ", "docker ",
    "kubectl ", "sudo ", "ssh ", "python ", "python3 ", "node ", "make ", "export ", "echo ",
    "mkdir ", "rm ", "go ", "bun ",
];

/// Heuristic: does this text read like source code or a shell command?
pub fn looks_like_code(text: &str, source_bundle: Option<&str>) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    let lines: Vec<&str> = t.lines().collect();
    let from_code_app =
        source_bundle.is_some_and(|b| CODE_APPS.contains(&b) || b.starts_with("com.jetbrains."));
    if lines.len() == 1 {
        let l = lines[0];
        if l.starts_with("$ ") || COMMANDS.iter().any(|c| l.starts_with(c)) {
            return true;
        }
        if from_code_app
            && (l.contains("()")
                || l.ends_with(';')
                || l.contains(" = ")
                || l.contains("=>")
                || l.contains("::"))
        {
            return true;
        }
        return (l.starts_with('{') && l.ends_with('}') && l.contains(':'))
            || (l.starts_with("<") && l.ends_with('>') && l.contains("</"));
    }
    let mut score = 0i32;
    let keywords = [
        "function ",
        "const ",
        "let ",
        "var ",
        "return ",
        "import ",
        "export ",
        "def ",
        "class ",
        "fn ",
        "pub ",
        "#include",
        "package ",
        "func ",
        "if (",
        "for (",
        "while (",
        "} else",
        "public ",
        "private ",
        "async ",
        "await ",
        "SELECT ",
        "=> ",
        "struct ",
        "impl ",
        "interface ",
        "type ",
    ];
    for l in &lines {
        let s = l.trim_end();
        if s.ends_with(';')
            || s.ends_with('{')
            || s.ends_with('}')
            || s.ends_with("};")
            || s.ends_with("):")
        {
            score += 2;
        }
        if l.starts_with("    ") || l.starts_with('\t') {
            score += 1;
        }
        let trimmed = s.trim_start();
        if keywords.iter().any(|k| trimmed.starts_with(k))
            || trimmed.starts_with("//")
            || trimmed.starts_with("# ") && from_code_app
        {
            score += 2;
        }
    }
    let prose = t
        .split_whitespace()
        .filter(|w| w.chars().all(char::is_alphabetic))
        .count() as f64
        / t.split_whitespace().count().max(1) as f64;
    let threshold = (lines.len() as i32).max(3);
    if from_code_app {
        score += lines.len() as i32;
    }
    score >= threshold && (prose < 0.85 || from_code_app)
}
