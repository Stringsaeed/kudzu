# Copy provenance

Source commit: `2352d319969bc753b2de1ca12ce76b7f8c3035c6`.

Plain copies: core and engine crates (including tests and fixtures), skills, design,
design scripts and their tests, formatted-paste.swift, icons, Paper Mono and its
OFL licence, Info.plist, iCloud entitlements, CloudKit schema, template and permission docs.
Only copied Rust edits: agent/mod.rs resolves the embedded skill inside this workspace
and its documentation names the new relative skill path.
Design scripts are historical exports awaiting standalone native export tooling;
they are not called by the build. Later source changes must be ported by hand before S75.
