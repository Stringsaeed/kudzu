import Foundation
import Testing
@testable import KudzuCore

// Contract tests for `KudzuCore/Models`: every fixture written by kudzu-core's `export_fixtures`
// example (or hand-copied from the Tauri crate's serde shapes) decodes, and re-encodes to the same
// JSON value. That proves key names, optional omission vs `null`, lowercase UUIDs and dates.

// MARK: - Round trips

/// One fixture file holding a JSON array of `T`.
struct ModelsFixture: Sendable, CustomTestStringConvertible {
    let file: String
    let roundTrip: @Sendable (Data, JSONEncoder) throws -> Data
    var testDescription: String { file }

    static func array<T: Codable & Sendable>(_ file: String, _: T.Type) -> ModelsFixture {
        ModelsFixture(file: file) { data, encoder in
            try encoder.encode(try JSONDecoder().decode([T].self, from: data))
        }
    }

    static func single<T: Codable & Sendable>(_ file: String, _: T.Type) -> ModelsFixture {
        ModelsFixture(file: file) { data, encoder in
            try encoder.encode(try JSONDecoder().decode(T.self, from: data))
        }
    }

    static let all: [ModelsFixture] = [
        .single("library", Library.self),
        .array("clip-entries", ClipEntry.self),
        .array("clip-events", ClipEvent.self),
        .array("clipboard-infos", ClipboardInfo.self),
        .array("render-results", RenderResult.self),
        .array("form-fields", FormFieldSpec.self),
        .array("conflicts", TriggerConflict.self),
        .array("backups", BackupInfo.self),
        .array("import-summaries", ImportSummary.self),
        .array("espanso-detections", EspansoDetection.self),
        .array("settings", Settings.self),
        .array("update-states", UpdateState.self),
        .array("sync-statuses", SyncStatus.self),
        .array("agent-setups", AgentSetup.self),
        .array("app-infos", AppInfo.self),
        .array("engine-statuses", EngineStatus.self),
        .array("form-requests", FormRequest.self),
        .array("form-payloads", FormPayload.self),
        .array("expansion-events", ExpansionEvent.self),
        .array("routes", Route.self),
    ]
}

func modelsFixture(_ name: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: name, withExtension: "json", subdirectory: "Fixtures/models"))
    return try Data(contentsOf: url)
}

@Suite struct ModelsRoundTripTests {
    /// Plain Foundation coders: the models carry the contract themselves.
    @Test(arguments: ModelsFixture.all)
    func roundTripsWithFoundationCoders(_ fixture: ModelsFixture) throws {
        let data = try modelsFixture(fixture.file)
        let again = try fixture.roundTrip(data, JSONEncoder())
        #expect(try JSONValue.parse(again) == JSONValue.parse(data))
    }

    /// `KudzuJSON.encoder()` (pretty, sorted) writes the same values.
    @Test(arguments: ModelsFixture.all)
    func roundTripsWithKudzuEncoder(_ fixture: ModelsFixture) throws {
        let data = try modelsFixture(fixture.file)
        let again = try fixture.roundTrip(data, KudzuJSON.encoder())
        #expect(try JSONValue.parse(again) == JSONValue.parse(data))
    }

    @Test func minimalLibraryTakesSerdeDefaults() throws {
        let library = try KudzuJSON.decode(Library.self, from: modelsFixture("library-minimal"))
        let snippet = try #require(library.snippets.first)
        #expect(library.formatVersion == 1)
        #expect(snippet.isEnabled && !snippet.isFavorite && snippet.triggers.isEmpty && snippet.name.isEmpty)
        #expect(snippet.fireMode == .immediate && snippet.caseMode == .propagate)
        #expect(library.groups.first?.isEnabled == true && library.groups.first?.sortIndex == 0)
        #expect(library.usage.first?.count == 0 && library.usage.first?.deviceID == "")
        // Uppercase ids decode; nanoseconds with an offset parse to the same instant.
        #expect(snippet.id.lowercased == "a1b2c3d4-e5f6-4a7b-8c9d-0000000000ff")
        #expect(abs(snippet.createdAt.timeIntervalSince1970 - 1_794_123_015.123_457) < 0.000_01)
        // Re-encoding matches what Rust writes after the same decode.
        let again = try KudzuJSON.encode(library)
        #expect(try JSONValue.parse(again) == JSONValue.parse(modelsFixture("library-minimal-normalized")))
    }

    @Test func libraryJSONIsSortedAndLowercase() throws {
        let library = try KudzuJSON.decode(Library.self, from: modelsFixture("library"))
        let text = String(decoding: try KudzuJSON.encode(library), as: UTF8.self)
        #expect(!text.contains("A1B2C3D4"))
        #expect(text.contains("\"https://kudzu.site/docs?a=1&b=2\""))
        let keys = text.split(separator: "\n").prefix(4).map { $0.trimmingCharacters(in: .whitespaces) }
        #expect(keys == ["{", "\"formatVersion\" : 1,", "\"groups\" : [", "{"])
    }
}

// MARK: - Encoding details

@Suite struct ModelsEncodingTests {
    private func object<T: Encodable>(_ value: T) throws -> [String: JSONValue] {
        guard case let .object(o) = try JSONValue.parse(JSONEncoder().encode(value)) else { throw CocoaError(.coderInvalidValue) }
        return o
    }

    @Test func uuidsEncodeLowercaseAndOptionalsOmit() throws {
        let id = try #require(UUID(uuidString: "ABCDEF01-2345-4678-9ABC-DEF012345678"))
        let snippet = Snippet(id: id, name: "x", createdAt: Date(timeIntervalSince1970: 0))
        let o = try object(snippet)
        #expect(o["id"] == .string("abcdef01-2345-4678-9abc-def012345678"))
        #expect(o["groupID"] == nil && o["formatting"] == nil && o["rule"] == nil && o["insertion"] == nil)
        #expect(o["createdAt"] == .string("1970-01-01T00:00:00Z"))

        var grouped = snippet
        grouped.groupID = id
        #expect(try object(grouped)["groupID"] == .string("abcdef01-2345-4678-9abc-def012345678"))
        #expect(try object(ClipEvent.remove(ids: [id]))["ids"] == .array([.string(id.lowercased)]))
        #expect(try object(Route.group(id: id))["id"] == .string(id.lowercased))
    }

    @Test func serdeOptionsWithoutSkipWriteNull() throws {
        #expect(try object(AppInfo(bundleId: "a", name: "A"))["pid"] == .null)
        #expect(try object(EngineStatus.starting)["secureInputOwner"] == .null)
        #expect(try object(FormFieldSpec(name: "n", kind: .text))["defaultValue"] == .null)
        #expect(try object(RenderResult(text: "t"))["cursorOffset"] == .null)
        #expect(try object(RenderResult(text: "t"))["formatting"] == nil)
        #expect(try object(Settings.default)["lastRunVersion"] == .null)
        #expect(try object(UpdateState.downloading(version: "1", downloaded: 0, total: nil))["total"] == .null)
    }

    @Test func clipEntryOmitsEmptyFiles() throws {
        let now = Date(timeIntervalSince1970: 1_800_000_000.25)
        let entry = ClipEntry(kind: .text, hash: "h", firstCopiedAt: now, copiedAt: now, timings: [now])
        let o = try object(entry)
        #expect(o["files"] == nil)
        #expect(o["copiedAt"] == .string("2027-01-15T08:00:00.250Z"))
        #expect(o["timings"] == .array([.string("2027-01-15T08:00:00.250Z")]))
    }

    @Test func unknownDiscriminatorsFail() {
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(FormatStyle.self, from: Data(#"{"kind":"strike"}"#.utf8)) }
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(SyncStatus.self, from: Data(#"{"state":"maybe"}"#.utf8)) }
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(Route.self, from: Data(#"{"kind":"nowhere"}"#.utf8)) }
    }

    @Test func appRuleDefaultsBundleIDs() throws {
        let rule = try JSONDecoder().decode(AppRule.self, from: Data(#"{"mode":"deny"}"#.utf8))
        #expect(rule == AppRule(mode: .deny))
    }
}

// MARK: - Settings defaults

@Suite struct ModelsSettingsTests {
    @Test func emptyObjectDecodesToDefaults() throws {
        #expect(try JSONDecoder().decode(Settings.self, from: Data("{}".utf8)) == Settings.default)
    }

    @Test func defaultsMatchRust() {
        let d = Settings.default
        #expect(d.typingSpeedWPM == 40 && d.defaultTriggerPrefix == ";" && d.paletteHotkey == "Control+Alt+Space")
        #expect(!d.paused && d.defaultInsertion == .automatic && !d.launchAtLogin && !d.onboardingCompleted)
        #expect(d.theme == .dark && d.showInDock && d.autoCheckUpdates && d.updateChannel == .stable && d.icloudSync)
        #expect(d.lastRunVersion == nil && d.clipboardEnabled && d.clipboardRetentionDays == 7)
        #expect(d.clipboardHotkey == "Control+Alt+V" && !d.clipboardPastePlain && !d.clipboardShowSecrets)
        #expect(!d.agentAccess && d.agentWrite && d.agentClipboard)
        // `MatchingConfig::default_denied_bundle_ids()`, sorted as `Settings::default` sorts them.
        #expect(d.deniedBundleIDs == [
            "com.1password.1password", "com.agilebits.onepassword-osx", "com.agilebits.onepassword7",
            "com.apple.Passwords", "com.apple.keychainaccess", "com.apple.loginwindow", "com.bitwarden.desktop",
        ])
        #expect(d.appInsertions.count == 7 && d.appInsertions.values.allSatisfy { $0 == .paste })
        #expect(d.appInsertions["com.mitchellh.ghostty"] == .paste)
        #expect(d.clipboardIgnoredApps.count == 12 && d.clipboardIgnoredApps.first == "com.1password.1password")
    }

    /// `Store::load_dev`: paused, with hotkeys that don't clash with a release build.
    @Test func devDefaultsMatchLoadDev() throws {
        var expected = Settings.default
        expected.paused = true
        expected.paletteHotkey = "Control+Alt+Shift+Space"
        expected.clipboardHotkey = "Control+Alt+Shift+V"
        #expect(Settings.devDefault == expected)
        #expect(try KudzuJSON.decoder(settingsDefaults: .devDefault).decode(Settings.self, from: Data("{}".utf8)) == .devDefault)
    }

    /// Port of `store.rs` `missing_settings_take_the_variant_defaults`.
    @Test func missingSettingsTakeTheVariantDefaults() throws {
        let file = Data(#"{"paused":false,"paletteHotkey":"Control+Alt+K"}"#.utf8)
        let dev = try KudzuJSON.decoder(settingsDefaults: .devDefault).decode(Settings.self, from: file)
        #expect(dev.clipboardHotkey == "Control+Alt+Shift+V")
        #expect(dev.paletteHotkey == "Control+Alt+K")
        #expect(!dev.paused)

        let prod = try KudzuJSON.decoder().decode(Settings.self, from: Data(#"{"paused":true}"#.utf8))
        #expect(prod.clipboardHotkey == "Control+Alt+V")
        #expect(prod.clipboardRetentionDays == 7)
        #expect(prod.paused)
    }

    @Test func explicitNullLastRunVersionIsNil() throws {
        var defaults = Settings.default
        defaults.lastRunVersion = "0.5.0"
        let decoder = KudzuJSON.decoder(settingsDefaults: defaults)
        #expect(try decoder.decode(Settings.self, from: Data(#"{"lastRunVersion":null}"#.utf8)).lastRunVersion == nil)
        #expect(try decoder.decode(Settings.self, from: Data("{}".utf8)).lastRunVersion == "0.5.0")
    }

    @Test func patchReplacesOnlyItsFields() {
        let patched = SettingsPatch { $0.theme = .light; $0.lastRunVersion = .some(nil) }.applied(to: .devDefault)
        var expected = Settings.devDefault
        expected.theme = .light
        #expect(patched == expected)
    }
}

// MARK: - Canonical JSON

/// A JSON value that keeps booleans and numbers apart (unlike `JSONSerialization`'s `NSNumber`).
enum JSONValue: Decodable, Equatable, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    static func parse(_ data: Data) throws -> JSONValue {
        try JSONDecoder().decode(JSONValue.self, from: data)
    }

    init(from decoder: any Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let b = try? c.decode(Bool.self) { self = .bool(b) }
        else if let n = try? c.decode(Double.self) { self = .number(n) }
        else if let s = try? c.decode(String.self) { self = .string(s) }
        else if let a = try? c.decode([JSONValue].self) { self = .array(a) }
        else { self = .object(try c.decode([String: JSONValue].self)) }
    }
}
