// Radius scale from `--radius: 0.625rem` (10 px at the 16 px root), as `@theme inline` derives it.

import CoreGraphics

/// Kudzu’s Radius design vocabulary.
public enum Radius {
    /// `--radius`.
    public static let base: CGFloat = 10
    /// `--radius-sm: calc(var(--radius) * 0.6)`.
    public static let sm: CGFloat = base * 0.6
    /// `--radius-md: calc(var(--radius) * 0.8)`.
    public static let md: CGFloat = base * 0.8
    /// `--radius-lg: var(--radius)`.
    public static let lg: CGFloat = base
    /// `--radius-xl: calc(var(--radius) * 1.4)`.
    public static let xl: CGFloat = base * 1.4
    /// `--radius-2xl: calc(var(--radius) * 1.8)`.
    public static let xl2: CGFloat = base * 1.8
    /// `--radius-3xl: calc(var(--radius) * 2.2)`.
    public static let xl3: CGFloat = base * 2.2
    /// `--radius-4xl: calc(var(--radius) * 2.6)`.
    public static let xl4: CGFloat = base * 2.6
    /// `rounded-full`.
    public static let full: CGFloat = 9999

    /// `rounded-[min(var(--radius-md),10px)]` (xs buttons, sm selects, sm toggle groups).
    public static let mdMax10: CGFloat = min(md, 10)
    /// `rounded-[min(var(--radius-md),12px)]` (sm buttons and toggles).
    public static let mdMax12: CGFloat = min(md, 12)
    /// `rounded-[5px]` (keycaps, trigger chips, app icon placeholders).
    public static let key: CGFloat = 5
    /// `rounded-[4px]` (checkbox).
    public static let checkbox: CGFloat = 4

    /// The native panels' corner radii (`windows.rs`): palette and clipboard 18, form 16.
    public static let palettePanel: CGFloat = 18
    /// The `formPanel` design-system value.
    public static let formPanel: CGFloat = 16
}
