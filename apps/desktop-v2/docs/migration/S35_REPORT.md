# S35 · System adapters: engine, pasteboard, apps

## Adapters (`crates/kudzu-system/src/`)

| Module | Type | Implements | Over |
|---|---|---|---|
| `engine` (+ `engine/{host,native}.rs`) | `EngineAdapter::new(Arc<EngineInstances>)` | `ExpansionEngine`, `FormChannel`, `KudzuWindows` | `kudzu_engine::Engine`, via a forwarding host |
| `pasteboard` | `SystemPasteboard::general()` | `Pasteboard` | NSPasteboard for reads; `kudzu_engine::clipboard` for writes and paste |
| `apps` | `SystemApps` | `AppCatalog`, `TargetApp`, `FileIcons` | moved enumeration and icons; `kudzu_engine::system` for focus, caret, selection |
| `thumbnails` | `ImageThumbnails` | `Thumbnailer` | moved NSImage code |
| `settings_panes` | `SettingsPanes` | `SystemSettings` | moved deep links; `kudzu_engine::system` for grants and prompts |
| `locale` | `locale::current()` | (no port) | moved NSLocale read |

`kudzu_engine::outgoing` is deleted. The engine dropped `plist` and the AppKit/Foundation features only that code used.

## Port changes (`crates/kudzu-app/src/ports/engine.rs`)

```rust
pub struct InputEvent { pub stamp: EventStamp, pub key_code: Option<u16> }
pub struct PanelFocus { pub key: bool, pub seq: u64, pub after: Option<InputEvent> }
#[derive(Default)] pub struct FormSubmission { pub event: Option<InputEvent>, pub focus_seq: u64 }
pub struct KudzuWindow { pub number: u32, pub passes_clicks_through: bool }

trait FormChannel {
    fn submit_form(&self, token: FormToken, values: HashMap<String, String>, submission: FormSubmission)
        -> Result<(), AdmissionError>;
    fn cancel_form(&self, token: FormToken);
    fn panel_focused(&self, token: FormToken, focus: PanelFocus);
}
pub trait KudzuWindows: Send + Sync { fn set_kudzu_windows(&self, windows: &[KudzuWindow]); }
```

`Ports` gains `windows: Arc<dyn KudzuWindows>`. `FormService::submit(token, values, FormSubmission)` and `FormService::panel_focused(token, PanelFocus)` replace the `EventStamp` forms. The port type is named `FormSubmission` because `forms::Submission` (the task result) already exists. `EngineEvents::trigger_lost` was already there.

## Decisions

- **Locale:** the host always passes a locale. S40 sets `RenderEnvironment.locale` from `kudzu_system::locale::current()`. The adapter also replaces an empty `EngineLibrary.locale` with it. The engine no longer reads the OS locale; its own fallback for an empty locale is the constant `en_US`.
- **Admission:** `NoEngine` before `start`, after `stop`, or once the instance is retired; `StaleToken` for another instance's token. The adapter never returns `Busy`. A native token that the engine no longer holds is admitted and produces no outcome, because `Engine::submit_form` returns `()`.
- **Fencing:** each adapter takes the next `EngineInstance` from the shared allocator when it is constructed. Constructing a newer adapter retires the older one: its callbacks are dropped and its `start` returns `Unavailable`. Restarting the same adapter keeps its namespace. `stop` refuses new admissions, stops the engine (its last status still reaches the sink) and then detaches the sink.
- **Pasteboard reads** don't take the engine's transaction. A read during a snippet paste sees the transient item, which carries the Concealed/Transient markers, so it is skipped.

## Contract suites (`crates/kudzu-app/src/fixtures/contract/`)

There is one file per port. Each fake runs its suite in `contract/fakes.rs`; each adapter runs the same functions:

- engine: `without_native` runs on the real `Engine`, never started. `with_native` runs on the adapter over a recording native fake.
- pasteboard: `reads` runs on a private `pasteboardWithUniqueName` board, including promised data that is never provided. `reads` + `writes` on the general board are `#[ignore]` (S71).
- apps, thumbnails, settings: read-only cases only. No activation, selection copy, prompt or pane is opened.

## For S40

- Keep one `Arc<EngineInstances>`. Stop the old `EngineAdapter` before you construct a new one. Hand the same adapter out as `engine`, `forms` and `windows`.
- After every `start`, call `expansion.resync()` (as before).
- Push `set_kudzu_windows` with every Kudzu window's `windowNumber` and click-through flag whenever one opens, closes or changes, before any form can open. A new adapter starts with an empty list.
- Put `locale::current()` in `RenderEnvironment.locale`.
- Run every pasteboard, apps and thumbnail call on workers. `write` can wait for an in-flight paste restore (about 0.5 s), `paste` up to about 850 ms, and `selection` up to 300 ms (it may press ⌘C).

## For S57

- Number key-status changes from 1 per form into `PanelFocus.seq`. Put `NSApp.currentEvent`'s CGEvent stamp and key code in `PanelFocus.after` and `FormSubmission.event` (`None` when there is no event, never invented).
- Pass the latest `seq` as `FormSubmission.focus_seq`.

## Open questions

- `Engine::submit_form` could return whether it accepted the token, so the adapter can answer `StaleToken` for a form the engine already dropped.
- An external copy that lands between the engine's restore write and its own-change record could be recorded as a new copy. That needs the engine to record the count inside the write; check it in S71.
