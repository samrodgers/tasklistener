# TaskListener — Spec

A lightweight desktop app (Windows + macOS) that listens passively, recognises the user's voice, and extracts tasks and commitments into a local list.

## Goals

- Capture tasks the user mentions in conversation without them having to type or stop what they're doing.
- Only react to the **user's own voice** — ignore other speakers, video, podcasts, meetings (unless the user is the one committing).
- Stay out of the way: low CPU/RAM, minimal UI, runs in the menu bar / system tray.
- Local-first. Audio never leaves the machine unless the user explicitly opts in.
- Push captured tasks out to the user's existing task system(s) — Apple Reminders, Microsoft To Do, Todoist, Things, Google Tasks — without forcing them to live inside this app.

## Non-goals (v1)

- Meeting transcription / minutes.
- Multi-user / shared lists.
- Mobile.
- Calendar scheduling, reminders with times — just capture for now.

## User experience

### Onboarding
1. Install, launch.
2. Grant microphone permission.
3. **Voice enrolment**: read 3–4 short prompts (~30 seconds total) so the app can build a speaker embedding for "me". User can re-enrol any time from settings.
4. Done. App lives in menu bar / tray and starts listening immediately. Always-on is the only mode; pause controls (below) handle the cases where the user wants it off.

### Steady state
- A small icon in the menu bar / tray shows status: idle / listening / processing.
- Click to open a popover with the current task list.
- Each task shows: text, time captured, source snippet ("from: '…remind me to send Jen the contract tomorrow…'"), and quick actions (✓ done, edit, delete).
- A toast/notification appears briefly when a new task is captured, with an Undo.
- Global hotkey to open the list. Optional hotkey for "ignore the last 30 seconds".

### Privacy controls
- Pause / resume listening from the menu bar (one click).
- "Pause for 30 min" / "Pause until I quit" presets.
- Indicator is always visible when the mic is active.
- All audio buffers are kept in memory only and discarded after transcription unless the user enables a debug log.

## What counts as a "task or commitment"

Examples the extractor should catch:
- "I need to send Alex the report by Friday."
- "Remind me to book the flights."
- "I'll pick up milk on the way home."
- "Let me circle back on that pricing question tomorrow."

Examples it should ignore:
- Questions ("Should I send Alex the report?").
- Hypotheticals ("If we had more time, I'd refactor this.").
- Statements about other people ("Alex is going to send the report.").
- Past-tense ("I sent Alex the report.").

The extractor should normalise tasks to imperative form ("Send Alex the report") and capture any time/date hint as a separate field.

## Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ Mic capture  │ ──▶ │ VAD + speaker│ ──▶ │ Speech-to-   │ ──▶ │ Task         │
│ (16kHz mono) │     │ verification │     │ text (local) │     │ extraction   │
└──────────────┘     └──────────────┘     └──────────────┘     └──────┬───────┘
                                                                       │
                                                              ┌────────▼───────┐
                                                              │ SQLite store + │
                                                              │ menu-bar UI    │
                                                              └────────────────┘
```

### 1. Audio capture
- Cross-platform via [cpal](https://github.com/RustAudio/cpal) (Rust) or platform-native APIs (CoreAudio on macOS, WASAPI on Windows).
- 16 kHz mono, 30 ms frames.

### 2. Voice activity detection (VAD)
- [Silero VAD](https://github.com/snakers4/silero-vad) (small ONNX model, ~1 MB, runs on CPU in real time).
- Only forward audio to downstream stages when speech is detected. Idle cost ~0%.

### 3. Speaker verification
- Enrolment produces an embedding for the user (e.g. ECAPA-TDNN via [SpeechBrain](https://speechbrain.github.io) ONNX export, or pyannote).
- Each VAD-segmented utterance is embedded and cosine-compared to the enrolled vector.
- **Relaxed threshold** (cosine ~0.55–0.60 rather than 0.70+). Errs on the side of capturing the user even when tired, whispering, eating, or on a bad mic. The downside — occasionally capturing a similar-sounding speaker — is mitigated by the task-extraction layer rejecting non-task utterances anyway, and by the per-task Undo in the UI.
- Threshold is tunable in settings ("strict / balanced / relaxed"); default = relaxed.

### 4. Speech-to-text
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) with the `small` or `base.en` model. Runs locally, no network.
- Apple Silicon → Metal; Intel/Windows → CPU or CUDA if available.
- Streaming transcription on speech segments only (not continuous).

### 5. Task extraction
- **Local LLM only**, fully offline. No cloud option in v1.
- **Model: Qwen2.5-3B-Instruct**, Q4_K_M quantised via llama.cpp (~2 GB on disk, ~2.5 GB RAM in use).
  - Chosen because at the 3B class it has the strongest instruction-following and structured-output (JSON) behaviour among permissively licensed open models as of early 2026, and runs comfortably on a 5-year-old laptop.
  - Alternatives considered: Llama 3.2 3B Instruct (close second, slightly weaker on JSON discipline); Phi-3.5 Mini (good but more verbose); Gemma 2 2B (smaller but noticeably weaker at structured extraction).
  - Model is downloaded on first launch from a pinned URL with a SHA-256 check, not bundled in the installer.
- Inference via [llama.cpp](https://github.com/ggerganov/llama.cpp) — Metal on Apple Silicon, CPU/CUDA on Windows. Grammar-constrained decoding (GBNF) to guarantee valid JSON output.
- Prompt returns structured JSON: `{is_task: bool, task: string|null, due_hint: string|null, confidence: float}`.
- Discard low-confidence results silently. Borderline ones surface in a "review" tab rather than the main list.
- The model swap is a single config change, so we can re-evaluate every few months as the open ecosystem moves.

### 6. Storage
- SQLite at `~/Library/Application Support/TaskListener/tasks.db` (mac) / `%APPDATA%\TaskListener\tasks.db` (Windows).
- Schema:
  - `tasks(id, text, due_hint, source_snippet, captured_at, status, confidence)`
  - `task_destinations(id, task_id, provider, external_id, external_url, pushed_at, last_error, state)` — one row per (task, destination) pair, so a single task can be pushed to multiple systems.
  - `providers(id, name, enabled, config_json, last_synced_at)` — one row per configured integration (e.g. one Todoist account, one Microsoft 365 account, Apple Reminders).
  - `settings(key, value)`
- OAuth tokens and API keys are **not** stored in SQLite. They live in the OS secret store: macOS Keychain, Windows Credential Manager. SQLite holds only a reference id.

### 7. UI — native per platform

- **macOS: SwiftUI** menu-bar app (`MenuBarExtra`, macOS 13+). Popover window for the task list, standard mac notifications, EventKit ready for v2.
- **Windows: WinUI 3** (Windows App SDK) tray app with a flyout window. `NotifyIcon` for the tray, `AppNotification` for toasts, Win 10 1809+ supported.
- Both platforms: a single tray/menu-bar icon with idle/listening/processing states, a popover/flyout containing the task list, global hotkey for "open list" and "ignore last 30 s", standard pause controls.

### Sharing code across platforms

The audio + ML pipeline (capture, VAD, speaker verification, whisper, llama.cpp, SQLite) is written **once in Rust** and exposed as a C ABI dynamic library:

- macOS: `libtasklistener.dylib`, called from Swift via a thin Swift package wrapping the C header.
- Windows: `tasklistener.dll`, called from C# via P/Invoke.

This keeps the heavy, model-driven work in one codebase while letting each UI feel genuinely native. Estimated split: ~70% of the code is the shared Rust core; ~15% SwiftUI; ~15% WinUI.

## Recommended stack

- **Core**: Rust dynamic library — audio (cpal), VAD (Silero), speaker (ECAPA-TDNN ONNX), STT (whisper.cpp bindings), task extraction (llama.cpp bindings), SQLite (rusqlite). Exposes a small C ABI: `start()`, `stop()`, `pause(duration)`, `enrol(samples)`, `subscribe(callback)`, `list_tasks()`, `update_task(...)`, `delete_task(...)`.
- **macOS UI**: SwiftUI, `MenuBarExtra`, Swift Package wrapping the C header.
- **Windows UI**: WinUI 3 / Windows App SDK, C# with P/Invoke to the DLL.
- **Models**: Silero VAD + ECAPA-TDNN + whisper.cpp `base.en` + Qwen2.5-3B-Instruct (Q4_K_M).
- **Distribution**: signed + notarised `.dmg` (mac); signed `.msix` or `.msi` (Windows). Sparkle for mac auto-update; Windows App SDK / MS Store or a custom updater for Windows.

Estimated footprint:
- **Idle, model unloaded**: <50 MB RAM, <1% CPU.
- **Active utterance**: brief CPU spike, sub-second STT.
- **Model loaded**: ~2.5 GB resident for the LLM, +200 MB working set.

The LLM is **unloaded after 5 minutes of silence by default** (configurable). First task after a quiet period takes ~1–2 s longer while the model reloads from disk; subsequent tasks are immediate. This keeps the steady-state footprint well under the "lightweight" bar without making the active experience feel laggy.

## Integrations — pushing tasks out

Captured tasks can be pushed to one or more external task systems. Push is **one-way** (TaskListener → external) in this iteration; two-way sync is deferred. The local list remains the source of truth and shows the push state of each task.

### Supported providers

We deliberately avoid any auth flow that requires us to register and operate OAuth client credentials. Every provider in Tier 1 uses one of: a system permission (Apple), a local URL scheme (Things), a user-pasted personal API token, or a user-supplied URL (webhook). This keeps the project shippable without us running OAuth infrastructure, and it sidesteps the trust/verification review processes that Microsoft and Google now require for desktop OAuth apps.

Tier 1 — ships with the integrations release:

| Provider | Platforms | Auth | API |
|---|---|---|---|
| Apple Reminders | macOS only | system permission prompt | EventKit (in-process, no network) |
| Things 3 | macOS only | none (local URL scheme) | `things:///add?...` x-callback-url |
| Todoist | macOS + Windows | personal API token (user pastes from Todoist → Settings → Integrations → Developer) | Todoist REST API v2 |
| Notion | macOS + Windows | internal integration token (user creates an integration in `notion.so/my-integrations`, shares a database with it, pastes the token) | Notion API |
| Generic webhook | macOS + Windows | user-supplied URL + optional bearer header | HTTP POST of task JSON — covers Zapier, n8n, Make, IFTTT, home-grown |

Tier 2 — same token / PAT model, added incrementally based on demand: Linear (personal API key), Asana (personal access token), Trello (API key + token), TickTick (when they expose PAT — currently OAuth-only, so deferred), GitHub Issues (PAT), Obsidian (local URI / file write), OmniFocus (URL scheme on mac).

**Microsoft To Do and Google Tasks are explicitly deferred.** Both are OAuth-only with no personal-token fallback; shipping them would require us to register, verify, and maintain OAuth client apps. Workaround for users today: route via the webhook provider into Zapier/Make, which handle the OAuth on their side. We can revisit if a maintainer steps up to operate those OAuth clients.

### User experience

**Connect a provider** (Settings → Integrations):
1. User clicks "Connect Todoist" (etc.).
2. For token-based providers (Todoist, Notion, Linear, Asana, …): the connect sheet shows a short, provider-specific guide ("Open Todoist → Settings → Integrations → Developer → copy your API token") with a deep-link button that opens the relevant page, plus a single password field for the token. Token is validated with a test API call before saving.
3. For Apple Reminders: trigger the system reminders-access prompt.
4. For Things: no auth — just toggle on (Things must be installed).
5. For the generic webhook: user pastes a URL and optionally a bearer header.
6. **List/project picker is mandatory** — once connected, the user must explicitly pick the destination list/project before tasks will push. No silent defaulting to iCloud Reminders or "Inbox". The connect flow is not "complete" until this is set.

**Per-provider rules**:
- "Push tasks to this provider" — on/off.
- Default list/project.
- Optional tag/label to apply (e.g. `from:tasklistener`).
- Minimum confidence threshold — by default only high-confidence tasks auto-push; review-tab tasks don't.

**Per-task UI**:
- Each task in the list shows small badges for destinations it has been pushed to (✓ Reminders, ✓ Todoist).
- Failures show a ⚠ badge with hover detail; click to retry.
- Right-click → "Push to…" to send a task to a specific provider on demand, even if auto-push is off.
- Right-click → "Open in Todoist" (etc.) to jump to the task in the source app via `external_url`.

**Push timing**: configurable.
- **Immediate** (default) — push as soon as a task is captured.
- **Manual** — tasks accumulate; user clicks "Push N tasks" from the popover.
- **Batched** — push every N minutes.

### Architecture

A `TaskDestination` provider trait in the Rust core:

```rust
trait TaskDestination {
    fn id() -> &'static str;                       // "todoist", "msft_todo", ...
    fn display_name() -> &'static str;
    fn auth_kind() -> AuthKind;                    // ApiToken, SystemPermission, UrlScheme, Webhook
    async fn connect(&mut self) -> Result<()>;
    async fn list_targets(&self) -> Result<Vec<Target>>;  // lists / projects
    async fn push(&self, task: &Task, target: &Target) -> Result<PushResult>;
    async fn check(&self, ext: &ExternalRef) -> Result<RemoteState>;  // for completion echo, future
}
```

- All providers live behind this trait. Adding a new one is a single file plus registration.
- The push pipeline:
  1. Task created → enqueue (task_id, provider) jobs for each enabled provider.
  2. Worker thread pulls from the queue, calls `push()`, writes a `task_destinations` row with the result.
  3. Failures retry with exponential backoff (1 min, 5 min, 30 min, 2 h, then dead-letter and surface in UI).
  4. Network down → jobs sit in queue; resume when back online.
- The UI subscribes to push-state changes via the existing core callback channel, so badges update live.

### Credential storage

- Personal API tokens, integration tokens, and webhook bearer headers are written to **macOS Keychain** / **Windows Credential Manager** — never to SQLite, never to disk in plaintext.
- SQLite holds only a non-sensitive reference id pointing at the keychain entry.
- A token that suddenly returns 401/403 surfaces a "Reconnect Todoist" banner in Settings, never a popup mid-task-capture. Tasks queue up locally until the user pastes a fresh token.
- Settings → Integrations shows, per provider: connection status, the masked last 4 chars of the token, last successful push, and a "Replace token" button.

### Field mapping

Local task → external task:

| Local field | Maps to |
|---|---|
| `text` | Title |
| `due_hint` (string like "Friday") | Provider-supports-natural-language? Forward as-is (Todoist parses these). Otherwise, drop into the description and skip the due date. v3 will resolve to a real datetime locally before push. |
| `source_snippet` | Description / notes (with a "Captured by TaskListener at …" footer) |
| `captured_at` | (informational only, not pushed) |
| `confidence` | (informational only, not pushed) |

### Failure & dedupe

- Every push records `external_id` and `external_url` in `task_destinations`. Re-pushing an already-pushed (task, provider) pair is a no-op unless the user explicitly chooses "Push again".
- If the local task is deleted, the v2 default is **don't touch the external task** — the user might already be working on it elsewhere. A setting "also delete from external systems" can flip this.
- Marking the local task done is the same: by default, leave the external task alone. Setting flip enables completion echo.

## Roadmap

**v1:** capture → extract → local list. Pause controls, voice enrolment, native menu-bar / tray UI.

**v1.1:** review tab for low-confidence captures, edit/merge/split tasks, export to CSV/Markdown.

**v2 — integrations (this section):** Tier 1 providers (Apple Reminders, Things 3, Todoist, Notion, generic webhook) with one-way push, multi-destination support, token-based auth in the OS keychain, retry/backoff, push-state badges in UI, mandatory destination-list picker on connect.

**v2.1:** Tier 2 providers (Linear, Asana, Trello, GitHub Issues, Obsidian, OmniFocus). Microsoft To Do / Google Tasks remain deferred until OAuth client operation is solved.

**v3:** local datetime resolution for `due_hint`, two-way sync (completion echo, edits flowing back), recurring tasks, project/tag inference, multi-device sync of the local list.
