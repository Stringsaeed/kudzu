# kudzu-core

Domain records, rules and algorithms. No UI, async runtime or platform adapters.
The application layer owns services, persistence and coordination; the engine owns
the keystroke path. File access is confined to interchange readers and writers,
including the temporary orchestration modules noted below.

| Module | Responsibility |
| --- | --- |
| `library` | `model`, deterministic `json`, group inheritance in `groups`, `triggers`, `conflicts`, `usage`, `dedupe`, `naming`, `starter` |
| `template` | Parser, serializer, renderer, analysis, ICU dates and `forms`; editor modules reserved for S23 |
| `matching` | Key buffer, trigger index, matcher, case propagation, undo, matching `config` and insertion precedence |
| `formatting` | Persisted spans and styles, validation and HTML output |
| `interchange` | Espanso, Raycast, TextExpander and rich-text conversion; pure `detect`, `merge` and `backup` rules |
| `clipboard` | Entry model, `classify`, `history`, `retention`, `blob`; S22 owns `query`, `sections`, `facts` |
| `secrets` | Detection and redaction, split into token formats, labelled values and password shape rules |
| `sync` | Records, payloads, local index and merge rules; no CloudKit calls |
| `activity` | In-memory event ring and explanations; no production serialization |
| `system` | `AppInfo`, `Rect`, `PermissionStatus`, `TapMode`, `EngineStatus` vocabulary for application ports |
| `search` | Fuzzy ranking: `index`, `rank`, `highlight`; ICU collation (`icu_collator`) for tie-breaks, crate-private |
| `color` | S24: colour parsing, representations and contrast |
| `keys` | S24: accelerator grammar, symbols, spoken labels and recorder rules |
| `release_notes` | S24: release-note presentation |
| `fmt` | S24: count, duration, date, byte-size and clock labels |

The crate root re-exports library records. Domain entry points re-export their
public types and operations; implementation modules stay private. `template::forms`
is also available directly. Cross-domain calls use the owning domain's path.

## Follow-on ownership

- S21 filled `search` (`search/{fuzzy,collate}.rs`) and extended `library/{groups,usage}.rs`
  (group tree, paths, counts, `usage_by_id`, `totals`). The search index takes the template
  summariser as a function. Callers pass `template::summary::plain_summary` at 2,000 units.
- S22 fills `clipboard/{query,sections,facts}.rs`.
- S23 filled `template/{doc,chips,presets,summary,lists,images}.rs`. `template/doc/{load,edit,format,clipboard}.rs`
  are part of the editor document, not separate public modules.
- S24 fills `color.rs`, `keys.rs`, `release_notes.rs` and `fmt.rs`.
- S25 uses `system` in the application ports. S20's engine scope permits import
  and type-name changes only, so its existing status structs remain until S34
  adopts these types. S39 should keep protocol tests independent of that relocation.
- S29 moved `interchange::coordinator` and `BackupStore` to `kudzu-app::data`. The pure
  detection, merge, naming, daily-backup and rotation rules stay here
  (`interchange::{detect,merge,backup}`).
- S32 moved the MCP inbound adapter to `kudzu-app::agents`. Form-field and
  diagnostic serialization remains for that protocol.

Every reserved file is declared already; later steps need not edit parent modules.

## Format and test contracts

Library records, formatting, clipboard entries, sync payloads and form memory keep
their persisted serialization. `tests/golden_formats.rs` checks six unchanged
fixtures, including backup names. The MCP adapter keeps its serialized fields and
diagnostics. Activity, render actions/results and import-warning serialization is
test-only: the copied assertions still run, without a production serialization API.
The two copied suites that need these derives live in `tests/unit/` and are included
only by the library's unit-test target.

The `base64` crate handles image encoding and decoding. Input normalization retains
the original reader's whitespace, unlimited terminal padding, nonzero trailing bits
and dangling-sextet acceptance; symbols after padding and invalid symbols still fail.

From the workspace root, compare copied test names, tokenized bodies, all fixture
bytes and production file lengths against the golden-fixture commit:

```sh
python3 crates/kudzu-core/scripts/check-restructure.py <golden-fixture-commit>
```

The script normalizes only explicitly listed relocated paths and the matcher type
rename. Run Cargo's tests separately to verify that the inventoried tests execute.
