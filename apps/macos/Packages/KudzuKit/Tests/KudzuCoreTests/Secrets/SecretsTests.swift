import Foundation
import Testing
@testable import KudzuCore

@Suite struct SecretsTests {
    @Test func ordinaryTextIsUntouched() {
        for text in ["Best regards,\nSaeed","Meeting moved to 3pm tomorrow, see https://example.com/agenda?id=42","hunter","getElementById","README.md","config.v2.json","node_modules","user@example.com","/Users/saeed/Projects/App2/src/Main.tsx","#FF00aa","$1,000,000","550e8400-e29b-41d4-a716-446655440000","9fceb02d0ae598e95dc970b74767f19372d61af8","let max_tokens = 1024;","password: string","const password = process.env.PASSWORD","Password: ********","iPhone15","The author: Jane Doe","Use Bearer auth"] { #expect(Secrets.redact(text).isClean,"\(text)") }
    }
    @Test func vendorKeys() {
        for key in ["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","github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123456789ABCDEF","sk-ant-api03-abcDEF123456ghiJKL789012mnoPQR345678","sk-proj-Abc123Def456Ghi789Jkl012Mno345","sk_live_4eC39HqLyjWDarjtT1zdp7dc","xoxb-123456789012-1234567890123-AbCdEfGhIjKlMnOpQrStUvWx","AKIAIOSFODNN7EXAMPLE","AIzaSyDdI0hCZtE6vySjMm-WEfRq3CPzqKqqsHI","glpat-xxxxxxxxxxxxxxxx2xxx","npm_abcdefghijklmnopqrstuvwxyz0123456789"] { #expect(Secrets.redact("my key is \(key) ok").text == "my key is [redacted: API key] ok") }
    }
    @Test func privateKeyBlock() {
        #expect(Secrets.redact("id:\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----\nthanks").text == "id:\n[redacted: private key]\nthanks")
        #expect(!Secrets.containsSecret("-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----"))
        #expect(Secrets.redact("-----BEGIN RSA PRIVATE KEY-----\nMIIE").text == "[redacted: private key]")
    }
    @Test func jwtAndAuthorization() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        #expect(Secrets.redact("token \(jwt)").text == "token [redacted: access token]")
        #expect(Secrets.redact("curl -H 'Authorization: Bearer abc.def-123_xyz'").text == "curl -H 'Authorization: Bearer [redacted: access token]'")
        #expect(Secrets.redact("Authorization: token deadbeefcafe").text == "Authorization: token [redacted: access token]")
    }
    @Test func labelledValues() {
        for (text,expected) in [("DB_PASSWORD=correct-horse","DB_PASSWORD=[redacted: password]"),(#"{"api_key": "abcd1234", "name": "x"}"#,#"{"api_key": "[redacted: secret]", "name": "x"}"#),("export GITHUB_TOKEN=abc123xyz","export GITHUB_TOKEN=[redacted: secret]"),("Wi-Fi password: purple monkey","Wi-Fi password: [redacted: password] monkey"),("The password is: Summer2024","The password is: [redacted: password]"),("API Key: q8w7e6r5","API Key: [redacted: secret]"),("--password=hunter22 --user bob","--password=[redacted: password] --user bob"),("pin: 4821","pin: [redacted: password]"),("client_secret => 'zz9plural'","client_secret => '[redacted: secret]'")] { #expect(Secrets.redact(text).text == expected) }
    }
    @Test func urlCredentialsAndWebhooks() {
        #expect(Secrets.redact("postgres://admin:s3cr3t@db.local:5432/app").text == "postgres://admin:[redacted: secret]@db.local:5432/app")
        #expect(!Secrets.containsSecret("https://user@example.com/path"))
        #expect(Secrets.redact("hook https://hooks.slack.com/services/T000/B000/XXXXXXXX done").text == "hook https://hooks.slack.com/services/[redacted: secret] done")
    }
    @Test func lonePasswords() {
        for pw in ["P@ssw0rd!","Tr0ub4dor&3","kezwy4-dyxbeR-mosmac","qT7xLm9vR2bN","a8f3k2m9x7q1z5w4"] { let r = Secrets.redact(pw); #expect(r.whole); #expect(r.text == "[redacted: password]") }
        #expect(Secrets.redact("  P@ssw0rd!\n").whole); #expect(Secrets.redact("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY").whole)
    }
    @Test func randomRunsInsideText() {
        #expect(Secrets.redact("secret is wJalrXUtnFEMI/K7MDENG/bPxRfiCY3XAMPL9KEY ok").text == "secret is [redacted: API key] ok")
        #expect(Secrets.redact("hex 7f3a9c2e5b8d1f4a6c0e9b2d5f8a1c4e").text == "hex [redacted: API key]")
        #expect(!Secrets.containsSecret("fn handle_incoming_connection_request(stream: UnixStream) {}"))
    }
    @Test func mixedTextIsNotWhole() {
        let r = Secrets.redact("password=abcd1234 for the staging box"); #expect(!r.whole); #expect(r.kinds == [.password])
        #expect(Secrets.redact("sk_live_4eC39HqLyjWDarjtT1zdp7dc\n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").whole)
    }
    @Test func handlesNonAscii() {
        #expect(!Secrets.containsSecret("Grüße aus Köln — schöne Woche! 🌿"))
        #expect(Secrets.redact("Passwort-Hinweis — password: Zürich#2024").text == "Passwort-Hinweis — password: [redacted: password]")
        #expect(Secrets.scan("🌿 password: Zürich#2024").first?.range.lowerBound == 15)
    }
    @Test func detectionLimitUsesUTF8Boundary() { let prefix = String(repeating:"é",count:Secrets.DETECT_LIMIT/2); #expect(Secrets.detect(prefix+"password: hunter22") == nil); #expect(Secrets.containsSecret(prefix+" password: hunter22")) }

    // MARK: Review: byte/scalar semantics (see Fixtures/secrets/corpus.json "adversarial")
    @Test func combiningMarkAfterLeadingSpaceDoesNotCrash() {
        // The grapheme-based port measured the leading space plus its mark, ran past the end and trapped.
        #expect(Secrets.redact(" \u{301}Tr0ub4dor&3").text == " [redacted: password]")
    }
    @Test func delimitersFollowedByCombiningMarksStillDelimit() {
        #expect(Secrets.redact("password: abc,\u{301}def ghi").isClean)        // `,` ends the value (3 chars)
        #expect(Secrets.redact("password: $\u{301}HOMEVALUE").isClean)         // `$…` is a placeholder
        #expect(!Secrets.containsSecret("https://host/\u{301}user:pw@x"))       // `/` ends the authority
        #expect(!Secrets.containsSecret("/\u{301}Tr0ub4dor&3"))                 // a path
    }
    @Test func numericMeansGeneralCategoryN() {
        // `char::is_numeric` is Nd/Nl/No; CJK numerals are letters with a numeric value, so they count as symbols.
        #expect(Secrets.redact("Abcdef\u{4e00}\u{4e8c}").whole)
    }
    @Test func urlEndsAtEveryUnicodeWhitespace() {
        #expect(Secrets.redact("https://user:pw@host\u{b}x:y@z").text == "https://user:[redacted: secret]@host\u{b}x:y@z")
        #expect(Secrets.scan("a://\u{3000}").isEmpty) // Rust panics here; Swift must not
    }
    @Test func webhookHostsCompareASCIILowercase() { #expect(Secrets.redact("https://hoo\u{212a}s.slack.com/services/T0/B0/XXXX").isClean) }
    @Test func overlappingFormatsAreRescanned() {
        #expect(Secrets.scan("xeyJxxxxxxxxxxxxx.eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl") == [Secrets.Finding(range: 18..<67, kind: .token)])
        #expect(Secrets.scan("-----BEGIN -----BEGIN RSA PRIVATE KEY-----\nMIIE").isEmpty)
    }
    @Test func scanIsLinearInSeparators() {
        // The first port rebuilt the whole prefix as a String at every `:`/`=`.
        let text = String(repeating: "key: value, ", count: 20_000)
        let elapsed = ContinuousClock().measure { _ = Secrets.scan(text) }
        #expect(elapsed < .seconds(3))
    }
}
