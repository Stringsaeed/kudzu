// GENERATED FILE — do not edit.
// Source: design/kudzu/project/tokens.json
// Regenerate: python3 scripts/design/generate-tokens.py
// swiftlint:disable all

import CoreGraphics
import SwiftUI

/// Metadata about the generated tokens.
public enum KzTokens {
    /// SHA-256 of tokens.json plus the generator script; a test compares it to catch a stale file.
    public static let sourceFingerprint = "cc8910a5501e51a924e08f6814495dfc0b56710b7f6ef394fd530def49ffcfcf"
    /// The design system's name and version from tokens.json.
    public static let designSystem = "Kudzu v1"
}

// MARK: - Colour

/// Every colour token in the design system, with its Light and Dark values and usage note.
public enum KzColorToken: String, CaseIterable, Sendable {
    /// The desktop-facing base under the window chrome and behind glass in mockups. Never a text ground inside the window.
    case canvas = "canvas"
    /// The list pane and settings background. Ground for ink and ink-muted.
    case surface = "surface"
    /// The detail pane, cards, inputs on focus, buttons. Ground for ink, ink-muted, vine, bloom, danger, warning.
    case surfaceRaised = "surface-raised"
    /// Wells: text fields at rest, search, segmented and toggle tracks, bar tracks. Ground for ink and ink-muted.
    case surfaceSunken = "surface-sunken"
    /// Translucent overlay for hovered rows, ghost buttons and icon buttons. Layer it over any surface.
    case surfaceHover = "surface-hover"
    /// Translucent overlay for the pressed state of the same controls.
    case surfacePressed = "surface-pressed"
    /// Hairlines: pane dividers, card borders, button borders, row separators in settings cards. Decorative, so no contrast floor.
    case line = "line"
    /// Stronger hairline for keycaps and the editor border on focus-less states.
    case lineStrong = "line-strong"
    /// Boundaries a control's state depends on (toggle-off track, toggle-off knob, checkbox). 3:1 on surface, surface-raised and surface-sunken in both themes.
    case edge = "edge"
    /// Primary text and icons on every surface, glass and vine-soft (14:1 or more in both themes).
    case ink = "ink"
    /// Secondary text: counts, descriptions, placeholders, metadata, idle icons. 5.4:1 or more on canvas, surface, surface-raised, surface-sunken and vine-soft in both themes.
    case inkMuted = "ink-muted"
    /// Disabled text and decorative glyphs only (empty-state mark). Never for information: it is about 3:1.
    case inkFaint = "ink-faint"
    /// The one accent, chartreuse, as a FILL: primary button, toggle on, the cover. Never as text or a thin mark in light (1.3:1 on white); use vine-ink for those.
    case vine = "vine"
    /// The accent as text, icons and marks: trigger prefixes, inline tags, the active sidebar icon, menu checks, usage bars, field focus edges. 4.6:1 or more on canvas, surface, surface-raised, surface-sunken and vine-soft in light; 11:1 or more in dark.
    case vineInk = "vine-ink"
    /// Hover fill of the primary button.
    case vineHover = "vine-hover"
    /// Selected rows in the snippet list and backups list, hovered menu items, inline tag background. Ground for ink and vine-ink.
    case vineSoft = "vine-soft"
    /// Text, icons and the toggle knob on a vine fill (13:1 in both themes). Dark in both themes: chartreuse always carries dark text.
    case onVine = "on-vine"
    /// Kudzu's flower. Marks favorites only: the star and the favorites badge. As text on surface-raised, surface and bloom-soft.
    case bloom = "bloom"
    /// Background of the bloom badge.
    case bloomSoft = "bloom-soft"
    /// Caution text and icons (an import that will overwrite, a trigger that clashes). On surface-raised and warning-soft; always with a word.
    case warning = "warning"
    /// Background of warning badges and inline notices.
    case warningSoft = "warning-soft"
    /// Destructive actions and errors. On surface-raised and danger-soft; always with a word or icon.
    case danger = "danger"
    /// Background of danger badges and error notices.
    case dangerSoft = "danger-soft"
    /// Translucent fill of trigger chips and keycaps, so they read on surface, surface-raised and vine-soft alike. Text on it is ink.
    case chip = "chip"
    /// Sidebar fill (84% opaque) under a backdrop blur of blur-glass. The fill, not the blur, carries contrast: ink everywhere, ink-muted only for counts and section labels.
    case glass = "glass"
    /// Floating layers: menus, popovers, the toolbar cluster (92% opaque). Ground for ink and ink-muted.
    case glassStrong = "glass-strong"
    /// The active sidebar item: a solid pill sitting on the glass. Ground for ink.
    case glassSelected = "glass-selected"
    /// The 1px top highlight inside every glass layer (in CSS it is part of shadow-glass).
    case glassStroke = "glass-stroke"
    /// The 1px outer edge of glass layers and the window.
    case glassEdge = "glass-edge"
    /// Keyboard focus ring: 2px solid, 2px offset. An alias of vine-ink, 4.6:1 or more on every surface in both themes.
    case focus = "focus"

    /// The Light theme value, with aliases resolved.
    public var light: KzRGBA {
        switch self {
        case .canvas: KzRGBA(0xE9, 0xEB, 0xE6, alpha: 0xFF)
        case .surface: KzRGBA(0xF7, 0xF8, 0xF5, alpha: 0xFF)
        case .surfaceRaised: KzRGBA(0xFF, 0xFF, 0xFF, alpha: 0xFF)
        case .surfaceSunken: KzRGBA(0xEF, 0xF1, 0xEC, alpha: 0xFF)
        case .surfaceHover: KzRGBA(0x15, 0x1A, 0x14, alpha: 0x0A)
        case .surfacePressed: KzRGBA(0x15, 0x1A, 0x14, alpha: 0x14)
        case .line: KzRGBA(0xE2, 0xE4, 0xDE, alpha: 0xFF)
        case .lineStrong: KzRGBA(0xC9, 0xCD, 0xC4, alpha: 0xFF)
        case .edge: KzRGBA(0x86, 0x8D, 0x82, alpha: 0xFF)
        case .ink: KzRGBA(0x15, 0x1A, 0x14, alpha: 0xFF)
        case .inkMuted: KzRGBA(0x59, 0x60, 0x56, alpha: 0xFF)
        case .inkFaint: KzRGBA(0x8C, 0x92, 0x88, alpha: 0xFF)
        case .vine: KzRGBA(0xC5, 0xF0, 0x3D, alpha: 0xFF)
        case .vineInk: KzRGBA(0x4A, 0x73, 0x00, alpha: 0xFF)
        case .vineHover: KzRGBA(0xB5, 0xE2, 0x2A, alpha: 0xFF)
        case .vineSoft: KzRGBA(0xEE, 0xF8, 0xD2, alpha: 0xFF)
        case .onVine: KzRGBA(0x14, 0x1A, 0x06, alpha: 0xFF)
        case .bloom: KzRGBA(0x9C, 0x2F, 0x86, alpha: 0xFF)
        case .bloomSoft: KzRGBA(0xF6, 0xE4, 0xF1, alpha: 0xFF)
        case .warning: KzRGBA(0x8A, 0x5A, 0x00, alpha: 0xFF)
        case .warningSoft: KzRGBA(0xFB, 0xF0, 0xD9, alpha: 0xFF)
        case .danger: KzRGBA(0xB3, 0x26, 0x1E, alpha: 0xFF)
        case .dangerSoft: KzRGBA(0xFB, 0xE7, 0xE5, alpha: 0xFF)
        case .chip: KzRGBA(0x15, 0x1A, 0x14, alpha: 0x0F)
        case .glass: KzRGBA(0xF3, 0xF4, 0xF0, alpha: 0xD6)
        case .glassStrong: KzRGBA(0xFB, 0xFB, 0xF9, alpha: 0xEB)
        case .glassSelected: KzRGBA(0xFF, 0xFF, 0xFF, alpha: 0xFF)
        case .glassStroke: KzRGBA(0xFF, 0xFF, 0xFF, alpha: 0xBF)
        case .glassEdge: KzRGBA(0x10, 0x14, 0x0F, alpha: 0x1A)
        case .focus: KzRGBA(0x4A, 0x73, 0x00, alpha: 0xFF)
        }
    }

    /// The Dark theme value, with aliases resolved.
    public var dark: KzRGBA {
        switch self {
        case .canvas: KzRGBA(0x0B, 0x0D, 0x0B, alpha: 0xFF)
        case .surface: KzRGBA(0x12, 0x15, 0x12, alpha: 0xFF)
        case .surfaceRaised: KzRGBA(0x17, 0x1A, 0x17, alpha: 0xFF)
        case .surfaceSunken: KzRGBA(0x0E, 0x10, 0x0E, alpha: 0xFF)
        case .surfaceHover: KzRGBA(0xE9, 0xEC, 0xE6, alpha: 0x0F)
        case .surfacePressed: KzRGBA(0xE9, 0xEC, 0xE6, alpha: 0x1A)
        case .line: KzRGBA(0x23, 0x28, 0x23, alpha: 0xFF)
        case .lineStrong: KzRGBA(0x3A, 0x41, 0x3A, alpha: 0xFF)
        case .edge: KzRGBA(0x6B, 0x72, 0x68, alpha: 0xFF)
        case .ink: KzRGBA(0xE9, 0xEC, 0xE6, alpha: 0xFF)
        case .inkMuted: KzRGBA(0x9A, 0xA1, 0x96, alpha: 0xFF)
        case .inkFaint: KzRGBA(0x63, 0x6A, 0x60, alpha: 0xFF)
        case .vine: KzRGBA(0xC5, 0xF0, 0x3D, alpha: 0xFF)
        case .vineInk: KzRGBA(0xC5, 0xF0, 0x3D, alpha: 0xFF)
        case .vineHover: KzRGBA(0xD4, 0xF8, 0x6C, alpha: 0xFF)
        case .vineSoft: KzRGBA(0x22, 0x2D, 0x0C, alpha: 0xFF)
        case .onVine: KzRGBA(0x14, 0x1A, 0x06, alpha: 0xFF)
        case .bloom: KzRGBA(0xE5, 0x8F, 0xD2, alpha: 0xFF)
        case .bloomSoft: KzRGBA(0x2C, 0x16, 0x28, alpha: 0xFF)
        case .warning: KzRGBA(0xF0, 0xC0, 0x60, alpha: 0xFF)
        case .warningSoft: KzRGBA(0x2A, 0x21, 0x10, alpha: 0xFF)
        case .danger: KzRGBA(0xFF, 0x8B, 0x7E, alpha: 0xFF)
        case .dangerSoft: KzRGBA(0x2E, 0x15, 0x13, alpha: 0xFF)
        case .chip: KzRGBA(0xE9, 0xEC, 0xE6, alpha: 0x14)
        case .glass: KzRGBA(0x14, 0x17, 0x14, alpha: 0xD6)
        case .glassStrong: KzRGBA(0x1B, 0x1F, 0x1B, alpha: 0xEB)
        case .glassSelected: KzRGBA(0xFF, 0xFF, 0xFF, alpha: 0x17)
        case .glassStroke: KzRGBA(0xFF, 0xFF, 0xFF, alpha: 0x12)
        case .glassEdge: KzRGBA(0x00, 0x00, 0x00, alpha: 0xA6)
        case .focus: KzRGBA(0xC5, 0xF0, 0x3D, alpha: 0xFF)
        }
    }

    /// The token this one is an alias of in tokens.json (`focus` → `vineInk`), if any.
    public var aliasOf: KzColorToken? {
        switch self {
        case .focus: .vineInk
        default: nil
        }
    }

    /// The token whose value replaces this one under Increase Contrast (not in tokens.json; see the generator).
    public var highContrastSubstitute: KzColorToken? {
        switch self {
        case .inkFaint: .inkMuted
        case .line: .lineStrong
        case .lineStrong: .edge
        default: nil
        }
    }

    /// Whether this token becomes fully opaque under Increase Contrast.
    public var isOpaqueInHighContrast: Bool {
        switch self {
        case .glass, .glassStrong: true
        default: false
        }
    }

    /// How the design system says this colour should be used.
    public var usage: String {
        switch self {
        case .canvas: "The desktop-facing base under the window chrome and behind glass in mockups. Never a text ground inside the window."
        case .surface: "The list pane and settings background. Ground for ink and ink-muted."
        case .surfaceRaised: "The detail pane, cards, inputs on focus, buttons. Ground for ink, ink-muted, vine, bloom, danger, warning."
        case .surfaceSunken: "Wells: text fields at rest, search, segmented and toggle tracks, bar tracks. Ground for ink and ink-muted."
        case .surfaceHover: "Translucent overlay for hovered rows, ghost buttons and icon buttons. Layer it over any surface."
        case .surfacePressed: "Translucent overlay for the pressed state of the same controls."
        case .line: "Hairlines: pane dividers, card borders, button borders, row separators in settings cards. Decorative, so no contrast floor."
        case .lineStrong: "Stronger hairline for keycaps and the editor border on focus-less states."
        case .edge: "Boundaries a control's state depends on (toggle-off track, toggle-off knob, checkbox). 3:1 on surface, surface-raised and surface-sunken in both themes."
        case .ink: "Primary text and icons on every surface, glass and vine-soft (14:1 or more in both themes)."
        case .inkMuted: "Secondary text: counts, descriptions, placeholders, metadata, idle icons. 5.4:1 or more on canvas, surface, surface-raised, surface-sunken and vine-soft in both themes."
        case .inkFaint: "Disabled text and decorative glyphs only (empty-state mark). Never for information: it is about 3:1."
        case .vine: "The one accent, chartreuse, as a FILL: primary button, toggle on, the cover. Never as text or a thin mark in light (1.3:1 on white); use vine-ink for those."
        case .vineInk: "The accent as text, icons and marks: trigger prefixes, inline tags, the active sidebar icon, menu checks, usage bars, field focus edges. 4.6:1 or more on canvas, surface, surface-raised, surface-sunken and vine-soft in light; 11:1 or more in dark."
        case .vineHover: "Hover fill of the primary button."
        case .vineSoft: "Selected rows in the snippet list and backups list, hovered menu items, inline tag background. Ground for ink and vine-ink."
        case .onVine: "Text, icons and the toggle knob on a vine fill (13:1 in both themes). Dark in both themes: chartreuse always carries dark text."
        case .bloom: "Kudzu's flower. Marks favorites only: the star and the favorites badge. As text on surface-raised, surface and bloom-soft."
        case .bloomSoft: "Background of the bloom badge."
        case .warning: "Caution text and icons (an import that will overwrite, a trigger that clashes). On surface-raised and warning-soft; always with a word."
        case .warningSoft: "Background of warning badges and inline notices."
        case .danger: "Destructive actions and errors. On surface-raised and danger-soft; always with a word or icon."
        case .dangerSoft: "Background of danger badges and error notices."
        case .chip: "Translucent fill of trigger chips and keycaps, so they read on surface, surface-raised and vine-soft alike. Text on it is ink."
        case .glass: "Sidebar fill (84% opaque) under a backdrop blur of blur-glass. The fill, not the blur, carries contrast: ink everywhere, ink-muted only for counts and section labels."
        case .glassStrong: "Floating layers: menus, popovers, the toolbar cluster (92% opaque). Ground for ink and ink-muted."
        case .glassSelected: "The active sidebar item: a solid pill sitting on the glass. Ground for ink."
        case .glassStroke: "The 1px top highlight inside every glass layer (in CSS it is part of shadow-glass)."
        case .glassEdge: "The 1px outer edge of glass layers and the window."
        case .focus: "Keyboard focus ring: 2px solid, 2px offset. An alias of vine-ink, 4.6:1 or more on every surface in both themes."
        }
    }
}

/// Kudzu's colours as dynamic SwiftUI colours that follow Light, Dark and Increase Contrast.
public enum KzColor {
    /// The desktop-facing base under the window chrome and behind glass in mockups. Never a text ground inside the window.
    public static let canvas: Color = KzColorToken.canvas.color
    /// The list pane and settings background. Ground for ink and ink-muted.
    public static let surface: Color = KzColorToken.surface.color
    /// The detail pane, cards, inputs on focus, buttons. Ground for ink, ink-muted, vine, bloom, danger, warning.
    public static let surfaceRaised: Color = KzColorToken.surfaceRaised.color
    /// Wells: text fields at rest, search, segmented and toggle tracks, bar tracks. Ground for ink and ink-muted.
    public static let surfaceSunken: Color = KzColorToken.surfaceSunken.color
    /// Translucent overlay for hovered rows, ghost buttons and icon buttons. Layer it over any surface.
    public static let surfaceHover: Color = KzColorToken.surfaceHover.color
    /// Translucent overlay for the pressed state of the same controls.
    public static let surfacePressed: Color = KzColorToken.surfacePressed.color
    /// Hairlines: pane dividers, card borders, button borders, row separators in settings cards. Decorative, so no contrast floor.
    public static let line: Color = KzColorToken.line.color
    /// Stronger hairline for keycaps and the editor border on focus-less states.
    public static let lineStrong: Color = KzColorToken.lineStrong.color
    /// Boundaries a control's state depends on (toggle-off track, toggle-off knob, checkbox). 3:1 on surface, surface-raised and surface-sunken in both themes.
    public static let edge: Color = KzColorToken.edge.color
    /// Primary text and icons on every surface, glass and vine-soft (14:1 or more in both themes).
    public static let ink: Color = KzColorToken.ink.color
    /// Secondary text: counts, descriptions, placeholders, metadata, idle icons. 5.4:1 or more on canvas, surface, surface-raised, surface-sunken and vine-soft in both themes.
    public static let inkMuted: Color = KzColorToken.inkMuted.color
    /// Disabled text and decorative glyphs only (empty-state mark). Never for information: it is about 3:1.
    public static let inkFaint: Color = KzColorToken.inkFaint.color
    /// The one accent, chartreuse, as a FILL: primary button, toggle on, the cover. Never as text or a thin mark in light (1.3:1 on white); use vine-ink for those.
    public static let vine: Color = KzColorToken.vine.color
    /// The accent as text, icons and marks: trigger prefixes, inline tags, the active sidebar icon, menu checks, usage bars, field focus edges. 4.6:1 or more on canvas, surface, surface-raised, surface-sunken and vine-soft in light; 11:1 or more in dark.
    public static let vineInk: Color = KzColorToken.vineInk.color
    /// Hover fill of the primary button.
    public static let vineHover: Color = KzColorToken.vineHover.color
    /// Selected rows in the snippet list and backups list, hovered menu items, inline tag background. Ground for ink and vine-ink.
    public static let vineSoft: Color = KzColorToken.vineSoft.color
    /// Text, icons and the toggle knob on a vine fill (13:1 in both themes). Dark in both themes: chartreuse always carries dark text.
    public static let onVine: Color = KzColorToken.onVine.color
    /// Kudzu's flower. Marks favorites only: the star and the favorites badge. As text on surface-raised, surface and bloom-soft.
    public static let bloom: Color = KzColorToken.bloom.color
    /// Background of the bloom badge.
    public static let bloomSoft: Color = KzColorToken.bloomSoft.color
    /// Caution text and icons (an import that will overwrite, a trigger that clashes). On surface-raised and warning-soft; always with a word.
    public static let warning: Color = KzColorToken.warning.color
    /// Background of warning badges and inline notices.
    public static let warningSoft: Color = KzColorToken.warningSoft.color
    /// Destructive actions and errors. On surface-raised and danger-soft; always with a word or icon.
    public static let danger: Color = KzColorToken.danger.color
    /// Background of danger badges and error notices.
    public static let dangerSoft: Color = KzColorToken.dangerSoft.color
    /// Translucent fill of trigger chips and keycaps, so they read on surface, surface-raised and vine-soft alike. Text on it is ink.
    public static let chip: Color = KzColorToken.chip.color
    /// Sidebar fill (84% opaque) under a backdrop blur of blur-glass. The fill, not the blur, carries contrast: ink everywhere, ink-muted only for counts and section labels.
    public static let glass: Color = KzColorToken.glass.color
    /// Floating layers: menus, popovers, the toolbar cluster (92% opaque). Ground for ink and ink-muted.
    public static let glassStrong: Color = KzColorToken.glassStrong.color
    /// The active sidebar item: a solid pill sitting on the glass. Ground for ink.
    public static let glassSelected: Color = KzColorToken.glassSelected.color
    /// The 1px top highlight inside every glass layer (in CSS it is part of shadow-glass).
    public static let glassStroke: Color = KzColorToken.glassStroke.color
    /// The 1px outer edge of glass layers and the window.
    public static let glassEdge: Color = KzColorToken.glassEdge.color
    /// Keyboard focus ring: 2px solid, 2px offset. An alias of vine-ink, 4.6:1 or more on every surface in both themes.
    public static let focus: Color = KzColorToken.focus.color
}

// MARK: - Type

/// Every type style in the design system. Apply one with `.kzTextStyle(_:)`.
public enum KzTextStyle: String, CaseIterable, Sendable {
    /// Big numbers on stat tiles. Tabular figures.
    case stat = "stat"
    /// Settings page titles and empty-state titles.
    case titleLg = "title-lg"
    /// Pane titles (list header) and snippet name in the detail header.
    case title = "title"
    /// Field labels in the editor, settings group titles, disclosure headers.
    case headline = "headline"
    /// Default UI text.
    case body = "body"
    /// Row titles: snippet names in the list, sidebar items, settings row labels, buttons.
    case bodyStrong = "body-strong"
    /// Snippet content in the editor and its preview: the user's own words get a reading size.
    case content = "content"
    /// Descriptions under settings rows and fields; small buttons.
    case callout = "callout"
    /// Counts, section labels, badges, metadata.
    case caption = "caption"
    /// Trigger chips and the trigger input. The signature face of the app (Geist Mono).
    case trigger = "trigger"
    /// Inline tags in content, shell snippets, keycaps.
    case code = "code"
    /// File names, device IDs, version strings.
    case monoCaption = "mono-caption"

    /// Size, line height, weight, tracking and family, straight from tokens.json.
    public var spec: KzTextSpec {
        switch self {
        case .stat: KzTextSpec(family: .sans, size: 28, lineHeight: 32, weight: 600, trackingEm: -0.02, tabularFigures: true, group: "Display", sample: "1,284")
        case .titleLg: KzTextSpec(family: .sans, size: 20, lineHeight: 26, weight: 600, trackingEm: -0.01, tabularFigures: false, group: "Display", sample: "Backups")
        case .title: KzTextSpec(family: .sans, size: 15, lineHeight: 20, weight: 600, trackingEm: -0.005, tabularFigures: false, group: "Text", sample: "All snippets")
        case .headline: KzTextSpec(family: .sans, size: 13, lineHeight: 18, weight: 600, trackingEm: 0, tabularFigures: false, group: "Text", sample: "Triggers")
        case .body: KzTextSpec(family: .sans, size: 13, lineHeight: 18, weight: 400, trackingEm: 0, tabularFigures: false, group: "Text", sample: "Choose a snippet to edit it.")
        case .bodyStrong: KzTextSpec(family: .sans, size: 13, lineHeight: 18, weight: 500, trackingEm: 0, tabularFigures: false, group: "Text", sample: "Launch Kudzu at login")
        case .content: KzTextSpec(family: .sans, size: 14, lineHeight: 22, weight: 400, trackingEm: 0, tabularFigures: false, group: "Text", sample: "Hi there, thanks for reaching out.")
        case .callout: KzTextSpec(family: .sans, size: 12, lineHeight: 16, weight: 400, trackingEm: 0, tabularFigures: false, group: "Text", sample: "Keeps the last 14 snapshots.")
        case .caption: KzTextSpec(family: .sans, size: 11, lineHeight: 14, weight: 500, trackingEm: 0.01, tabularFigures: false, group: "Text", sample: "Groups")
        case .trigger: KzTextSpec(family: .mono, size: 12, lineHeight: 16, weight: 500, trackingEm: 0, tabularFigures: false, group: "Mono", sample: ":espanso")
        case .code: KzTextSpec(family: .mono, size: 12, lineHeight: 18, weight: 400, trackingEm: 0, tabularFigures: false, group: "Mono", sample: "{date:%d %b %Y}")
        case .monoCaption: KzTextSpec(family: .mono, size: 11, lineHeight: 14, weight: 400, trackingEm: 0, tabularFigures: false, group: "Mono", sample: "kudzu-backup-2026-11-02.json")
        }
    }

    /// How the design system says this style should be used.
    public var usage: String {
        switch self {
        case .stat: "Big numbers on stat tiles. Tabular figures."
        case .titleLg: "Settings page titles and empty-state titles."
        case .title: "Pane titles (list header) and snippet name in the detail header."
        case .headline: "Field labels in the editor, settings group titles, disclosure headers."
        case .body: "Default UI text."
        case .bodyStrong: "Row titles: snippet names in the list, sidebar items, settings row labels, buttons."
        case .content: "Snippet content in the editor and its preview: the user's own words get a reading size."
        case .callout: "Descriptions under settings rows and fields; small buttons."
        case .caption: "Counts, section labels, badges, metadata."
        case .trigger: "Trigger chips and the trigger input. The signature face of the app (Geist Mono)."
        case .code: "Inline tags in content, shell snippets, keycaps."
        case .monoCaption: "File names, device IDs, version strings."
        }
    }
}

// MARK: - Spacing

/// Spacing tokens from the design system, in points.
public enum KzSpace {
    /// Icon-to-label inside chips; gap between toolbar buttons.
    public static let s1: CGFloat = 4
    /// Gap between a label and its control; chip padding; list row gap.
    public static let s2: CGFloat = 8
    /// Row padding (vertical 10, horizontal 12); button horizontal padding.
    public static let s3: CGFloat = 12
    /// Card padding; pane gutters; settings row horizontal padding.
    public static let s4: CGFloat = 16
    /// Gap between fields in the editor.
    public static let s5: CGFloat = 20
    /// Detail pane padding; gap between settings groups.
    public static let s6: CGFloat = 24
    /// Top padding of settings pages and empty states.
    public static let s8: CGFloat = 32
    /// Large separations only (empty-state offset).
    public static let s12: CGFloat = 48

    /// Every token in this group, in design-system order, for galleries and docs.
    public static let all: [KzMetric] = [
        KzMetric(token: "space-1", name: "s1", value: 4, usage: "Icon-to-label inside chips; gap between toolbar buttons."),
        KzMetric(token: "space-2", name: "s2", value: 8, usage: "Gap between a label and its control; chip padding; list row gap."),
        KzMetric(token: "space-3", name: "s3", value: 12, usage: "Row padding (vertical 10, horizontal 12); button horizontal padding."),
        KzMetric(token: "space-4", name: "s4", value: 16, usage: "Card padding; pane gutters; settings row horizontal padding."),
        KzMetric(token: "space-5", name: "s5", value: 20, usage: "Gap between fields in the editor."),
        KzMetric(token: "space-6", name: "s6", value: 24, usage: "Detail pane padding; gap between settings groups."),
        KzMetric(token: "space-8", name: "s8", value: 32, usage: "Top padding of settings pages and empty states."),
        KzMetric(token: "space-12", name: "s12", value: 48, usage: "Large separations only (empty-state offset)."),
    ]
}

// MARK: - Radius

/// Radius tokens from the design system, in points.
public enum KzRadius {
    /// Inline tags and keycaps.
    public static let xs: CGFloat = 4
    /// Trigger chips, small buttons, segmented options, menu items.
    public static let sm: CGFloat = 6
    /// Buttons, fields, sidebar items, list rows.
    public static let md: CGFloat = 8
    /// Cards, the editor, menus, the toolbar cluster.
    public static let lg: CGFloat = 12
    /// The window and full-height glass panels.
    public static let xl: CGFloat = 16
    /// Toggles, badges, usage bars.
    public static let full: CGFloat = 999

    /// Every token in this group, in design-system order, for galleries and docs.
    public static let all: [KzMetric] = [
        KzMetric(token: "radius-xs", name: "xs", value: 4, usage: "Inline tags and keycaps."),
        KzMetric(token: "radius-sm", name: "sm", value: 6, usage: "Trigger chips, small buttons, segmented options, menu items."),
        KzMetric(token: "radius-md", name: "md", value: 8, usage: "Buttons, fields, sidebar items, list rows."),
        KzMetric(token: "radius-lg", name: "lg", value: 12, usage: "Cards, the editor, menus, the toolbar cluster."),
        KzMetric(token: "radius-xl", name: "xl", value: 16, usage: "The window and full-height glass panels."),
        KzMetric(token: "radius-full", name: "full", value: 999, usage: "Toggles, badges, usage bars."),
    ]
}

// MARK: - Shadow

/// Every shadow token in the design system. Apply one with `.kzShadow(_:in:)`.
public enum KzShadow: String, CaseIterable, Sendable {
    /// Buttons, the selected segment, toggle knobs.
    case control = "shadow-control"
    /// The toolbar cluster and other small glass layers: the glass-stroke highlight plus a soft drop.
    case glass = "shadow-glass"
    /// Menus and popovers.
    case float = "shadow-float"
    /// The app window in mockups (macOS draws the real one).
    case window = "shadow-window"

    /// The CSS shadow list for the Light theme, parsed into layers (first layer on top).
    public var lightLayers: [KzShadowLayer] {
        switch self {
        case .control: [
            KzShadowLayer(inset: false, x: 0, y: 1, blur: 2, spread: 0, color: KzRGBA(16 / 255, 20 / 255, 15 / 255, 0.06)),
        ]
        case .glass: [
            KzShadowLayer(inset: true, x: 0, y: 1, blur: 0, spread: 0, color: KzRGBA(255 / 255, 255 / 255, 255 / 255, 0.75)),
            KzShadowLayer(inset: false, x: 0, y: 1, blur: 2, spread: 0, color: KzRGBA(16 / 255, 20 / 255, 15 / 255, 0.06)),
            KzShadowLayer(inset: false, x: 0, y: 8, blur: 24, spread: -10, color: KzRGBA(16 / 255, 20 / 255, 15 / 255, 0.18)),
        ]
        case .float: [
            KzShadowLayer(inset: true, x: 0, y: 1, blur: 0, spread: 0, color: KzRGBA(255 / 255, 255 / 255, 255 / 255, 0.75)),
            KzShadowLayer(inset: false, x: 0, y: 1, blur: 2, spread: 0, color: KzRGBA(16 / 255, 20 / 255, 15 / 255, 0.06)),
            KzShadowLayer(inset: false, x: 0, y: 16, blur: 40, spread: -8, color: KzRGBA(16 / 255, 20 / 255, 15 / 255, 0.22)),
        ]
        case .window: [
            KzShadowLayer(inset: false, x: 0, y: 0, blur: 0, spread: 0.5, color: KzRGBA(16 / 255, 20 / 255, 15 / 255, 0.14)),
            KzShadowLayer(inset: false, x: 0, y: 24, blur: 64, spread: -12, color: KzRGBA(16 / 255, 20 / 255, 15 / 255, 0.3)),
        ]
        }
    }

    /// The CSS shadow list for the Dark theme, parsed into layers (first layer on top).
    public var darkLayers: [KzShadowLayer] {
        switch self {
        case .control: [
            KzShadowLayer(inset: false, x: 0, y: 1, blur: 2, spread: 0, color: KzRGBA(0 / 255, 0 / 255, 0 / 255, 0.5)),
        ]
        case .glass: [
            KzShadowLayer(inset: true, x: 0, y: 1, blur: 0, spread: 0, color: KzRGBA(255 / 255, 255 / 255, 255 / 255, 0.07)),
            KzShadowLayer(inset: false, x: 0, y: 1, blur: 2, spread: 0, color: KzRGBA(0 / 255, 0 / 255, 0 / 255, 0.5)),
            KzShadowLayer(inset: false, x: 0, y: 12, blur: 32, spread: -10, color: KzRGBA(0 / 255, 0 / 255, 0 / 255, 0.7)),
        ]
        case .float: [
            KzShadowLayer(inset: true, x: 0, y: 1, blur: 0, spread: 0, color: KzRGBA(255 / 255, 255 / 255, 255 / 255, 0.07)),
            KzShadowLayer(inset: false, x: 0, y: 1, blur: 2, spread: 0, color: KzRGBA(0 / 255, 0 / 255, 0 / 255, 0.5)),
            KzShadowLayer(inset: false, x: 0, y: 16, blur: 40, spread: -8, color: KzRGBA(0 / 255, 0 / 255, 0 / 255, 0.75)),
        ]
        case .window: [
            KzShadowLayer(inset: false, x: 0, y: 0, blur: 0, spread: 0.5, color: KzRGBA(0 / 255, 0 / 255, 0 / 255, 0.9)),
            KzShadowLayer(inset: false, x: 0, y: 24, blur: 64, spread: -12, color: KzRGBA(0 / 255, 0 / 255, 0 / 255, 0.85)),
        ]
        }
    }

    /// How the design system says this shadow should be used.
    public var usage: String {
        switch self {
        case .control: "Buttons, the selected segment, toggle knobs."
        case .glass: "The toolbar cluster and other small glass layers: the glass-stroke highlight plus a soft drop."
        case .float: "Menus and popovers."
        case .window: "The app window in mockups (macOS draws the real one)."
        }
    }
}

// MARK: - Size

/// Size tokens from the design system, in points.
public enum KzSize {
    /// Small buttons, segmented options inside a 28px track.
    public static let controlSm: CGFloat = 24
    /// Buttons, toolbar icon buttons, search, sidebar items.
    public static let control: CGFloat = 30
    /// Text fields, select, trigger input.
    public static let field: CGFloat = 34
    /// Height of every pane's header row, aligned with the traffic lights.
    public static let titlebar: CGFloat = 52
    /// Sidebar width (glass).
    public static let sidebar: CGFloat = 220
    /// Snippet list / settings section list width.
    public static let list: CGFloat = 300

    /// Every token in this group, in design-system order, for galleries and docs.
    public static let all: [KzMetric] = [
        KzMetric(token: "size-control-sm", name: "controlSm", value: 24, usage: "Small buttons, segmented options inside a 28px track."),
        KzMetric(token: "size-control", name: "control", value: 30, usage: "Buttons, toolbar icon buttons, search, sidebar items."),
        KzMetric(token: "size-field", name: "field", value: 34, usage: "Text fields, select, trigger input."),
        KzMetric(token: "size-titlebar", name: "titlebar", value: 52, usage: "Height of every pane's header row, aligned with the traffic lights."),
        KzMetric(token: "size-sidebar", name: "sidebar", value: 220, usage: "Sidebar width (glass)."),
        KzMetric(token: "size-list", name: "list", value: 300, usage: "Snippet list / settings section list width."),
    ]
}

// MARK: - Blur

/// Blur tokens from the design system, in points.
public enum KzBlur {
    /// Sidebar and floating layers: backdrop-filter blur(28px) saturate(160%).
    public static let glass: CGFloat = 28

    /// Every token in this group, in design-system order, for galleries and docs.
    public static let all: [KzMetric] = [
        KzMetric(token: "blur-glass", name: "glass", value: 28, usage: "Sidebar and floating layers: backdrop-filter blur(28px) saturate(160%)."),
    ]
}
