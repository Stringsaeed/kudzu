// Colour tokens, transcribed from `apps/desktop/app/globals.css` (`:root`, `.dark`, `.vibrant`).
//
// "Dark first. Graphite neutrals with a faint cool cast, one accent ("iris") used only for focus,
// selection and tokens. Depth comes from 1px hairlines + very soft shadows, never from heavy
// borders. Light theme mirrors the same structure."
//
// Palette: shadcn theme (stone neutrals, blue primary). Kudzu-only tokens are derived below it.
// `var(--x)` references are resolved here, so every token is a literal OKLCH value per theme.

import Foundation

/// Every colour custom property in `globals.css`, by its CSS name.
public enum KZToken: String, CaseIterable, Sendable {
    case background, foreground
    case card, cardForeground = "card-foreground"
    case popover, popoverForeground = "popover-foreground"
    case primary, primaryForeground = "primary-foreground"
    case secondary, secondaryForeground = "secondary-foreground"
    case muted, mutedForeground = "muted-foreground"
    case accent, accentForeground = "accent-foreground"
    case destructive, border, input, ring
    case chart1 = "chart-1", chart2 = "chart-2", chart3 = "chart-3", chart4 = "chart-4", chart5 = "chart-5"
    case sidebar, sidebarForeground = "sidebar-foreground"
    case sidebarPrimary = "sidebar-primary", sidebarPrimaryForeground = "sidebar-primary-foreground"
    case sidebarAccent = "sidebar-accent", sidebarAccentForeground = "sidebar-accent-foreground"
    case sidebarBorder = "sidebar-border", sidebarRing = "sidebar-ring"
    // Kudzu extensions
    case surface, surfaceRaised = "surface-raised", hairline, faint
    case iris, irisSoft = "iris-soft", irisForeground = "iris-foreground", irisContrast = "iris-contrast"
    // Template chips
    case chipTime = "chip-time", chipClipboard = "chip-clipboard", chipCursor = "chip-cursor"
    case chipForm = "chip-form", chipError = "chip-error", chipNeutral = "chip-neutral"
    case success, warning, favorite
    /// `.vibrant` / `.vibrant.dark` `--sidebar`: the sidebar over `NSVisualEffectView`.
    case sidebarVibrant = "sidebar@vibrant"
    /// `scrollbar-color: oklch(0.5 0 0 / 0.25) transparent` (both themes).
    case scrollbarThumb = "scrollbar-thumb"

    /// The CSS custom property name, e.g. `--muted-foreground`.
    public var cssName: String { "--" + rawValue }
}

/// The light and dark OKLCH token values from the web design system.
public enum Palette {
    /// The token's value in light (`:root`) or dark (`.dark`) theme.
    public static func value(_ token: KZToken, dark: Bool) -> OKLCH {
        let pair = table[token] ?? (OKLCH(0, 0, 0), OKLCH(0, 0, 0))
        return dark ? pair.1 : pair.0
    }

    // MARK: Values (light, dark)

    private static let fgLight = OKLCH(0.147, 0.004, 49.25)
    private static let fgDark = OKLCH(0.985, 0.001, 106.423)
    private static let primaryLight = OKLCH(0.488, 0.243, 264.376)
    private static let primaryDark = OKLCH(0.424, 0.199, 265.638)
    private static let primaryFg = OKLCH(0.97, 0.014, 254.604)
    private static let cardDark = OKLCH(0.216, 0.006, 56.043)
    private static let sidebarPrimaryDark = OKLCH(0.623, 0.214, 259.815)
    private static let irisFgLight = OKLCH(0.488, 0.243, 264.376)
    private static let irisFgDark = OKLCH(0.809, 0.105, 251.813)

    static let table: [KZToken: (OKLCH, OKLCH)] = [
        .background: (OKLCH(1, 0, 0), OKLCH(0.147, 0.004, 49.25)),
        .foreground: (fgLight, fgDark),
        .card: (OKLCH(1, 0, 0), cardDark),
        .cardForeground: (fgLight, fgDark),
        .popover: (OKLCH(1, 0, 0), cardDark),
        .popoverForeground: (fgLight, fgDark),
        .primary: (primaryLight, primaryDark),
        .primaryForeground: (primaryFg, primaryFg),
        .secondary: (OKLCH(0.967, 0.001, 286.375), OKLCH(0.274, 0.006, 286.033)),
        .secondaryForeground: (OKLCH(0.21, 0.006, 285.885), OKLCH(0.985, 0, 0)),
        .muted: (OKLCH(0.97, 0.001, 106.424), OKLCH(0.268, 0.007, 34.298)),
        .mutedForeground: (OKLCH(0.553, 0.013, 58.071), OKLCH(0.709, 0.01, 56.259)),
        .accent: (OKLCH(0.97, 0.001, 106.424), OKLCH(0.268, 0.007, 34.298)),
        .accentForeground: (OKLCH(0.216, 0.006, 56.043), fgDark),
        .destructive: (OKLCH(0.577, 0.245, 27.325), OKLCH(0.704, 0.191, 22.216)),
        .border: (OKLCH(0.923, 0.003, 48.717), OKLCH(1, 0, 0, alpha: 0.10)),
        .input: (OKLCH(0.923, 0.003, 48.717), OKLCH(1, 0, 0, alpha: 0.15)),
        .ring: (OKLCH(0.709, 0.01, 56.259), OKLCH(0.553, 0.013, 58.071)),
        .chart1: (OKLCH(0.809, 0.105, 251.813), OKLCH(0.809, 0.105, 251.813)),
        .chart2: (OKLCH(0.623, 0.214, 259.815), OKLCH(0.623, 0.214, 259.815)),
        .chart3: (OKLCH(0.546, 0.245, 262.881), OKLCH(0.546, 0.245, 262.881)),
        .chart4: (OKLCH(0.488, 0.243, 264.376), OKLCH(0.488, 0.243, 264.376)),
        .chart5: (OKLCH(0.424, 0.199, 265.638), OKLCH(0.424, 0.199, 265.638)),
        .sidebar: (OKLCH(0.985, 0.001, 106.423), cardDark),
        .sidebarForeground: (fgLight, fgDark),
        .sidebarPrimary: (OKLCH(0.546, 0.245, 262.881), sidebarPrimaryDark),
        .sidebarPrimaryForeground: (primaryFg, primaryFg),
        .sidebarAccent: (OKLCH(0.97, 0.001, 106.424), OKLCH(0.268, 0.007, 34.298)),
        .sidebarAccentForeground: (OKLCH(0.216, 0.006, 56.043), fgDark),
        .sidebarBorder: (OKLCH(0.923, 0.003, 48.717), OKLCH(1, 0, 0, alpha: 0.10)),
        .sidebarRing: (OKLCH(0.709, 0.01, 56.259), OKLCH(0.553, 0.013, 58.071)),
        // --surface: var(--card)
        .surface: (OKLCH(1, 0, 0), cardDark),
        .surfaceRaised: (OKLCH(1, 0, 0), OKLCH(0.245, 0.006, 56)),
        .hairline: (OKLCH(0.147, 0.004, 49.25, alpha: 0.08), OKLCH(1, 0, 0, alpha: 0.08)),
        .faint: (OKLCH(0.65, 0.01, 56), OKLCH(0.58, 0.012, 56)),
        // --iris: var(--primary) in light; var(--sidebar-primary) in dark (the brighter blue for
        // icons, switches and selection, since dark primary is deep for buttons).
        .iris: (primaryLight, sidebarPrimaryDark),
        .irisSoft: (OKLCH(0.488, 0.243, 264.376, alpha: 0.1), OKLCH(0.623, 0.214, 259.815, alpha: 0.18)),
        .irisForeground: (irisFgLight, irisFgDark),
        // --iris-contrast: var(--primary-foreground)
        .irisContrast: (primaryFg, primaryFg),
        .chipTime: (OKLCH(0.47, 0.13, 235), OKLCH(0.8, 0.1, 220)),
        .chipClipboard: (OKLCH(0.47, 0.12, 158), OKLCH(0.8, 0.12, 158)),
        // --chip-cursor: var(--iris-foreground)
        .chipCursor: (irisFgLight, irisFgDark),
        .chipForm: (OKLCH(0.52, 0.13, 62), OKLCH(0.84, 0.11, 75)),
        .chipError: (OKLCH(0.52, 0.19, 24), OKLCH(0.75, 0.16, 24)),
        // --chip-neutral: var(--foreground)
        .chipNeutral: (fgLight, fgDark),
        .success: (OKLCH(0.6, 0.15, 155), OKLCH(0.76, 0.14, 158)),
        .warning: (OKLCH(0.68, 0.15, 70), OKLCH(0.8, 0.13, 75)),
        .favorite: (OKLCH(0.72, 0.16, 65), OKLCH(0.82, 0.14, 75)),
        // macOS: let NSVisualEffectView show through the sidebar (same colours, translucent).
        .sidebarVibrant: (OKLCH(0.985, 0.001, 106.423, alpha: 0.72), OKLCH(0.216, 0.006, 56.043, alpha: 0.55)),
        .scrollbarThumb: (OKLCH(0.5, 0, 0, alpha: 0.25), OKLCH(0.5, 0, 0, alpha: 0.25)),
    ]
}
