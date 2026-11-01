/// Global inputs to trigger matching that do not belong to a snippet or group.
public struct MatchingConfig: Hashable, Sendable {
    /// Characters that end a word, in addition to whitespace and newlines (always separators).
    public static let defaultSeparators: Set<Character> = [".", ",", "!", "?", ";", ":", ")", "]", "}", "'", "\""]

    /// Password managers and the login window, where expanding text would be wrong or unsafe.
    public static let defaultDeniedBundleIDs: Set<String> = [
        "com.1password.1password",
        "com.agilebits.onepassword7",
        "com.agilebits.onepassword-osx",
        "com.bitwarden.desktop",
        "com.apple.keychainaccess",
        "com.apple.Passwords",
        "com.apple.loginwindow",
    ]

    /// Common terminal emulators, where paste is preferred over synthesized keystrokes.
    public static let defaultAppInsertions: [String: InsertionMethod] = [
        "com.apple.Terminal": .paste,
        "com.googlecode.iterm2": .paste,
        "dev.warp.Warp-Stable": .paste,
        "com.mitchellh.ghostty": .paste,
        "org.alacritty": .paste,
        "net.kovidgoyal.kitty": .paste,
        "com.github.wez.wezterm": .paste,
    ]

    /// Insertion method used when neither the snippet, its groups, nor an app rule choose one.
    public var defaultInsertion: InsertionMethod
    /// Bundle IDs where nothing ever expands, regardless of snippet or group rules.
    public var deniedBundleIDs: Set<String>
    /// Global per-app insertion overrides (bundle ID → insertion method), applied when neither the
    /// snippet nor its group sets an insertion method.
    public var appInsertions: [String: InsertionMethod]
    /// Punctuation that ends a word for `.wholeWord` snippets; whitespace and newlines always do.
    public var separators: Set<Character>

    public init(
        defaultInsertion: InsertionMethod = .automatic,
        deniedBundleIDs: Set<String> = MatchingConfig.defaultDeniedBundleIDs,
        appInsertions: [String: InsertionMethod] = MatchingConfig.defaultAppInsertions,
        separators: Set<Character> = MatchingConfig.defaultSeparators
    ) {
        self.defaultInsertion = defaultInsertion
        self.deniedBundleIDs = deniedBundleIDs
        self.appInsertions = appInsertions
        self.separators = separators
    }

    /// Whether `character` ends a word: whitespace, a newline, or a configured separator.
    public func isSeparator(_ character: Character) -> Bool {
        character.isWhitespace || character.isNewline || separators.contains(character)
    }
}
