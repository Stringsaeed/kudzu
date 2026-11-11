use super::*;

fn kinds(text: &str) -> Vec<SecretKind> {
    redact(text).kinds
}

fn redacted(text: &str) -> String {
    redact(text).text
}

#[test]
fn ordinary_text_is_untouched() {
    for text in [
        "Best regards,\nSaeed",
        "Meeting moved to 3pm tomorrow, see https://example.com/agenda?id=42",
        "hunter",
        "getElementById",
        "README.md",
        "config.v2.json",
        "node_modules",
        "user@example.com",
        "/Users/saeed/Projects/App2/src/Main.tsx",
        "#FF00aa",
        "$1,000,000",
        "550e8400-e29b-41d4-a716-446655440000",
        "9fceb02d0ae598e95dc970b74767f19372d61af8",
        "let max_tokens = 1024;",
        "password: string",
        "const password = process.env.PASSWORD",
        "Password: ********",
        "iPhone15",
        "The author: Jane Doe",
        "Use Bearer auth",
    ] {
        assert!(
            kinds(text).is_empty(),
            "flagged {text:?} as {:?}",
            kinds(text)
        );
    }
}

#[test]
fn vendor_keys() {
    for key in [
        concat!("ghp", "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx"),
        concat!(
            "github",
            "_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123456789ABCDEF"
        ),
        concat!("sk-ant", "-api03-abcDEF123456ghiJKL789012mnoPQR345678"),
        concat!("sk-proj", "-Abc123Def456Ghi789Jkl012Mno345"),
        concat!("sk_live", "_4eC39HqLyjWDarjtT1zdp7dc"),
        concat!(
            "xoxb",
            "-123456789012-1234567890123-AbCdEfGhIjKlMnOpQrStUvWx"
        ),
        "AKIAIOSFODNN7EXAMPLE",
        concat!("AIza", "SyDdI0hCZtE6vySjMm-WEfRq3CPzqKqqsHI"),
        "glpat-xxxxxxxxxxxxxxxx2xxx",
        concat!("npm", "_abcdefghijklmnopqrstuvwxyz0123456789"),
    ] {
        let text = format!("my key is {key} ok");
        assert_eq!(redacted(&text), "my key is [redacted: API key] ok", "{key}");
    }
}

#[test]
fn private_key_block() {
    let text = "id:\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----\nthanks";
    assert_eq!(redacted(text), "id:\n[redacted: private key]\nthanks");
    // A public key or certificate is fine.
    assert!(kinds("-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----").is_empty());
    // An unterminated block is withheld to the end.
    assert_eq!(
        redacted("-----BEGIN RSA PRIVATE KEY-----\nMIIE"),
        "[redacted: private key]"
    );
}

#[test]
fn jwt_and_authorization() {
    let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
    assert_eq!(
        redacted(&format!("token {jwt}")),
        "token [redacted: access token]"
    );
    assert_eq!(
        redacted("curl -H 'Authorization: Bearer abc.def-123_xyz'"),
        "curl -H 'Authorization: Bearer [redacted: access token]'"
    );
    assert_eq!(
        redacted("Authorization: token deadbeefcafe"),
        "Authorization: token [redacted: access token]"
    );
}

#[test]
fn labelled_values() {
    assert_eq!(
        redacted("DB_PASSWORD=correct-horse"),
        "DB_PASSWORD=[redacted: password]"
    );
    assert_eq!(
        redacted(r#"{"api_key": "abcd1234", "name": "x"}"#),
        r#"{"api_key": "[redacted: secret]", "name": "x"}"#
    );
    assert_eq!(
        redacted("export GITHUB_TOKEN=abc123xyz"),
        "export GITHUB_TOKEN=[redacted: secret]"
    );
    assert_eq!(
        redacted("Wi-Fi password: purple monkey"),
        "Wi-Fi password: [redacted: password] monkey"
    );
    assert_eq!(
        redacted("The password is: Summer2024"),
        "The password is: [redacted: password]"
    );
    assert_eq!(redacted("API Key: q8w7e6r5"), "API Key: [redacted: secret]");
    assert_eq!(
        redacted("--password=hunter22 --user bob"),
        "--password=[redacted: password] --user bob"
    );
    assert_eq!(redacted("pin: 4821"), "pin: [redacted: password]");
    assert_eq!(
        redacted("client_secret => 'zz9plural'"),
        "client_secret => '[redacted: secret]'"
    );
}

#[test]
fn url_credentials_and_webhooks() {
    assert_eq!(
        redacted("postgres://admin:s3cr3t@db.local:5432/app"),
        "postgres://admin:[redacted: secret]@db.local:5432/app"
    );
    assert!(kinds("https://user@example.com/path").is_empty());
    assert_eq!(
        redacted("hook https://hooks.slack.com/services/T000/B000/XXXXXXXX done"),
        "hook https://hooks.slack.com/services/[redacted: secret] done"
    );
}

#[test]
fn lone_passwords() {
    for pw in [
        "P@ssw0rd!",
        "Tr0ub4dor&3",
        "kezwy4-dyxbeR-mosmac",
        "qT7xLm9vR2bN",
        "a8f3k2m9x7q1z5w4",
    ] {
        let r = redact(pw);
        assert!(r.whole, "{pw} not flagged");
        assert_eq!(r.text, "[redacted: password]");
    }
    // Surrounding whitespace keeps it whole.
    assert!(redact("  P@ssw0rd!\n").whole);
    // A generated key alone is whole too.
    assert!(redact("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY").whole);
}

#[test]
fn random_runs_inside_text() {
    assert_eq!(
        redacted("secret is wJalrXUtnFEMI/K7MDENG/bPxRfiCY3XAMPL9KEY ok"),
        "secret is [redacted: API key] ok"
    );
    assert_eq!(
        redacted("hex 7f3a9c2e5b8d1f4a6c0e9b2d5f8a1c4e"),
        "hex [redacted: API key]"
    );
    // Long code identifiers stay.
    assert!(kinds("fn handle_incoming_connection_request(stream: UnixStream) {}").is_empty());
}

#[test]
fn mixed_text_is_not_whole() {
    let r = redact("password=abcd1234 for the staging box");
    assert!(!r.whole);
    assert_eq!(r.kinds, vec![SecretKind::Password]);
    let r = redact(concat!(
        "sk_live",
        "_4eC39HqLyjWDarjtT1zdp7dc\nghp",
        "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx"
    ));
    assert!(r.whole);
}

#[test]
fn handles_non_ascii() {
    assert!(kinds("Grüße aus Köln — schöne Woche! 🌿").is_empty());
    assert_eq!(
        redacted("Passwort-Hinweis — password: Zürich#2024"),
        "Passwort-Hinweis — password: [redacted: password]"
    );
}
