import AppKit
import CoreText
import Synchronization

/// Registers the bundled Geist and Geist Mono faces and resolves type styles to fonts.
public enum KzFonts {
    /// What happened when the fonts were registered.
    public enum Status: Sendable, Equatable {
        /// `register()` has not run yet.
        case notRegistered
        /// Every bundled face is available.
        case registered
        /// Some faces failed to register; styles fall back to the system font for those.
        case failed(missing: [String])
    }

    /// The PostScript names of the bundled faces.
    public static let postScriptNames: [String] = [
        "Geist-Regular", "Geist-Medium", "Geist-SemiBold",
        "GeistMono-Regular", "GeistMono-Medium", "GeistMono-SemiBold",
    ]

    private static let state = Mutex<Status>(.notRegistered)

    /// Registers the bundled fonts for this process. Idempotent and thread-safe; call it once at launch.
    ///
    /// Type-style APIs call this lazily too, so forgetting it only costs the first lookup.
    @discardableResult
    public static func register() -> Status {
        state.withLock { status in
            if status == .registered { return status }
            let urls = (Bundle.module.urls(forResourcesWithExtension: "otf", subdirectory: "Fonts") ?? [])
                .sorted { $0.lastPathComponent < $1.lastPathComponent }
            for url in urls {
                var error: Unmanaged<CFError>?
                if !CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error), let error {
                    // "Already registered" is success for our purposes (another copy of the bundle did it).
                    let code = CFErrorGetCode(error.takeRetainedValue())
                    if code != CTFontManagerError.alreadyRegistered.rawValue {
                        continue
                    }
                }
            }
            let missing = postScriptNames.filter { NSFont(name: $0, size: 12) == nil }
            status = missing.isEmpty ? .registered : .failed(missing: missing)
            return status
        }
    }

    /// The registration result so far.
    public static var status: Status {
        state.withLock { $0 }
    }

    /// The PostScript name for a family and CSS weight, snapping to the bundled 400/500/600.
    public static func postScriptName(family: KzFontFamily, weight: Int) -> String {
        let face = switch weight {
        case ..<450: "Regular"
        case 450..<550: "Medium"
        default: "SemiBold"
        }
        return (family == .sans ? "Geist-" : "GeistMono-") + face
    }

    /// Whether a bundled face is usable right now (registering first if needed).
    public static func isAvailable(_ postScriptName: String) -> Bool {
        register()
        return NSFont(name: postScriptName, size: 12) != nil
    }
}
