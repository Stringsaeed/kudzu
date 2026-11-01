/// How the user capitalised a typed trigger.
public enum CaseStyle: Hashable, Sendable {
    /// Leave the expansion untouched.
    case asIs
    /// First letter uppercase, e.g. `;Sig`.
    case capitalized
    /// Two or more letters, all uppercase, e.g. `;SIG`.
    case upper
}

/// Detects the case a trigger was typed in and applies it to expansion text.
public enum CasePropagation {
    /// Style of `typedTrigger`, judged on its cased letters only (punctuation, digits and
    /// uncased scripts are ignored): all uppercase with ≥2 letters → `.upper`, first letter
    /// uppercase → `.capitalized`, otherwise `.asIs`.
    public static func detect(typedTrigger: String) -> CaseStyle {
        var letterCount = 0
        var firstIsUpper = false
        var allUpper = true
        for ch in typedTrigger where ch.isLetter && ch.isCased {
            if letterCount == 0 { firstIsUpper = ch.isUppercase }
            if !ch.isUppercase { allUpper = false }
            letterCount += 1
        }
        if letterCount >= 2 && allUpper { return .upper }
        if letterCount >= 1 && firstIsUpper { return .capitalized }
        return .asIs
    }

    /// Applies `style` to expansion `text`. `.capitalized` uppercases only the first letter
    /// (skipping leading whitespace and punctuation; text starting with a digit is untouched).
    public static func apply(_ style: CaseStyle, to text: String) -> String {
        switch style {
        case .asIs:
            return text
        case .upper:
            return text.uppercased()
        case .capitalized:
            guard let index = text.firstIndex(where: { $0.isLetter || $0.isNumber }),
                  text[index].isLetter
            else { return text }
            var result = text
            result.replaceSubrange(index...index, with: text[index].uppercased())
            return result
        }
    }
}

/// Case folding shared by matching and conflict detection.
enum TriggerFolding {
    /// Lowercases one character without changing the character count.
    static func fold(_ character: Character) -> Character {
        if let ascii = character.asciiValue {
            return (ascii >= 65 && ascii <= 90) ? Character(Unicode.Scalar(ascii + 32)) : character
        }
        let lowered = character.lowercased()
        return lowered.count == 1 ? lowered.first! : character
    }

    static func fold(_ string: String) -> String {
        String(string.map(fold))
    }
}
