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
    ///
    /// Like Rust's `HashSet<char>`, each separator is a Unicode scalar: a multi-scalar character
    /// contributes each of its scalars.
    public var separators: Set<Character> {
        didSet { separatorScalars = Self.scalars(of: separators) }
    }
    /// `separators` as scalars, compared exactly (not by canonical equivalence: U+037E is not `;`).
    private var separatorScalars: Set<Unicode.Scalar>

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
        self.separatorScalars = Self.scalars(of: separators)
    }

    /// Whether `character` ends a word: whitespace, a newline, or a configured separator.
    ///
    /// A single scalar is judged on its own; a multi-scalar cluster (e.g. CRLF, or `.` with a
    /// combining mark) only by whether its first scalar is whitespace, as in `kudzu-core`.
    public func isSeparator(_ character: Character) -> Bool {
        let scalars = character.unicodeScalars
        guard let first = scalars.first else { return false }
        if scalars.count > 1 { return first.properties.isWhitespace }
        return first.properties.isWhitespace || separatorScalars.contains(first)
    }

    private static func scalars(of separators: Set<Character>) -> Set<Unicode.Scalar> {
        Set(separators.flatMap(\.unicodeScalars))
    }
}
