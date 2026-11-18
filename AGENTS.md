# Working on Anywhere

Read [README.md](README.md) for product status and
[docs/README.md](docs/README.md) for current contracts. This file is a code map,
not a second product specification. Prefer current code and topic guides over
old plans; dated records under `docs/archive/` describe earlier implementations.

## Boundaries

| Area | Owns | Entry points |
|---|---|---|
| Rust Core | Context, Action, matching, ranking and execution contracts | `crates/anywhere-core/src/core/` |
| Application | Session orchestration, cancellation, snapshots and service wiring | `crates/anywhere-core/src/application/` |
| Shared frontend runtime | GPUI-facing Application, settings, usage and Web document ownership | `crates/anywhere-core/src/application/frontend_runtime/` |
| Features | Domain behavior and statically registered first-party capabilities | `crates/anywhere-core/src/features/` |
| Persistence | SQLite, bounded source reads and stored preferences | `crates/anywhere-core/src/persistence/` |
| Platform | OS capabilities, application identity, paths and native operations | `crates/anywhere-platform/src/` |
| GPUI | Pages, input, selection, focus, settings, tool surfaces and window presentation | `apps/gpui/src/` |
| macOS bridge | Thin AppKit boundaries for floating windows, status item, Quick Look, login item and WebKit | `apps/gpui/src/platform/` (`platform/macos/`, `platform/material/`), `apps/gpui/src/host/` |
| Web plugins | Plugin business logic through the shared SDK and theme | `packages/plugin-sdk/`, `plugins/examples/` |

First-party features are compiled modules with registration contributions, not
hot-loaded plugins. Third-party plugins use the existing package/grant/SDK
boundary: WebView for UI, on-demand QuickJS for finite background jobs, and the
same Rust capability broker. Third-party Context types must stay in
`plugin.<package-id>.*`; use the existing Context registry and never convert
script data into native file authority. Legacy declarative TOML plugins have been retired.

## Find the behavior before editing

- Product entry: `apps/gpui/src/app.rs` (with window render owners in `launcher/tool_windows.rs`). It owns the main GPUI window, global
  hotkey dispatch, status-item restoration and the independent Clipboard Shelf
  and Running Tasks windows. The product has one UI implementation and one presentation stack.
- Launcher navigation, retention and selection:
  `apps/gpui/src/launcher/model.rs`, `launcher/view/` (`controllers/`, `pages/`),
  `launcher/snapshot_gate.rs`, `application/session_*` (`session_coordinator.rs`, `session_runtime/`) and
  [launcher-interaction.md](docs/launcher-interaction.md).
- Action ranking: `core/action/matcher.rs`, `core/ranking/` and
  `core/search/`. Empty action lists compare usage count, last use, relevance
  and stable IDs. GPUI only filters the chosen scope and limits visible rows.
  Keyword search, file results and source history lists have separate contracts.
- File search scanning: `features/filesystem/scan.rs` owns query lifetime; its
  `query`, `scope`, `record`, `traversal`, `storage`, `progress` and `pagination` modules
  own matching, scope checks, record format, enumeration, byte budgets and result delivery.
  `result_store.rs` keeps continuous external sorting; see
  [file-search.md](docs/file-search.md).
- Installed application index: `application/application_index.rs` owns scan
  generations, full/filtered catalogs, exclusions and completion
  notifications; `session_runtime.rs` revalidates its immutable snapshot
  before publication.
- Activation Context reads: `application/context_capture.rs` owns source
  policy, frozen application reads and late capture publication; its
  `clipboard.rs` owns clipboard requests, age and fact publication.
  `activate.rs` joins these with Git and Usage refreshes without sharing the
  capture policy lock.
- Committed execution requests: `application/execution_request.rs` freezes the
  target, source and authorization before queueing; its `worker.rs` runs
  blocking dispatch, Web/process handoff and Usage recording. `execute.rs` maps
  results and menu recovery.
- Feature catalog publication: `application/feature_catalog.rs` owns
  definitions, providers, candidate sources, policy, catalog versions and
  startup binding claims; Application revokes Session / Web execution under
  the same publication locks.
- Application history: source packages in
  `crates/anywhere-core/resources/application-history/`; catalog and transforms
  in `application/application_history/`; state and authority in
  `features/application_history/`; object actions in
  `application/history_object_actions.rs`; UI in
  `apps/gpui/src/launcher/view/` and `host/runtime/`. See
  [the source guide](docs/application-history-sources.md).
- Settings close decisions, drafts, authorization and native effects live in
  `apps/gpui/src/launcher/view/settings.rs`, `launcher/view/settings/` (`controller.rs` and per-category modules),
  `host/native_window.rs`, `host/preferences.rs` and
  `host/runtime/`; domain-to-GPUI snapshot projection belongs in
  `host/runtime_support.rs`. Hotkey parsing belongs in `host/hotkeys.rs`;
  login-item reconciliation in `host/login_item.rs`; directory bookmarks in
  `host/directory_access.rs`.
- Native Quick Look ownership lives in `apps/gpui/src/platform/preview.rs`.
  Web document policy and IPC framing live in `apps/gpui/src/web_tool.rs`;
  GPUI WebView mounting and lifecycle live in `launcher/view/` and
  `launcher/view/pages/`.
- A new first-party capability should contribute through its registration,
  registry and `HostView::NativeTool` interfaces instead of adding an
  application-specific branch to the Launcher.

Three kinds of history are intentionally separate: Launcher **Usage** stores
action counts; **application history** reads one selected external source;
**clipboard history** persists explicitly enabled local clipboard content.
Do not merge them into a global activity log.

## Preserve these contracts

- Scope filters actions; Context determines applicability. Current App
  aggregates available domains and includes menus without a separate menu
  Scope.
- Unavailable menus are hidden. Missing Accessibility permission removes menus
  without blocking other actions. Menu identity and legacy usage remain
  quarantined as described in
  [current-app-actions.md](docs/current-app-actions.md).
- Version checks belong immediately before async publication and execution.
  A stable action ID alone never authorizes an old candidate or changed target.
- Keep files, directories, workspace files and remote URIs distinct. Do not
  infer a workspace's first directory or choose an ambiguous application.
- `ActionResult.close_window` hides the GPUI window only after preview and Web
  document cleanup. A hidden session keeps input for 15 seconds; side effects
  are revoked immediately.
- Clipboard History is a normal in-window tool. Clipboard Shelf is a separate
  always-on-top GPUI window with its own search, copy, preview, delete and
  refresh controls.
- Web documents are reachable only through their Rust-owned handle. A late
  IPC reply, state event or close callback from a replaced document must not
  affect the current surface.

## Shared UI defaults

The GPUI app owns product pages. Shared values come from
`resources/design-tokens.json`; Rust projections are generated into
`apps/gpui/src/ui/design_tokens.rs`. File-type icons and filters come from
`resources/file-types.json` (`apps/gpui/src/launcher/file_icons.rs`). User-facing strings come from
`resources/localization/`; the typed Rust projection is
`apps/gpui/src/l10n/generated.rs`. Web documents receive
`resources/web-bootstrap.js` and `resources/web-controls.css` automatically.

Use ordinary GPUI components first and customize only actual layout or semantic
exceptions. Keep colors, spacing, typography and control shapes in shared
tokens instead of one-use literals. Do not introduce nested app shells or a
second UI framework.

## Verify and document

Use `rg` to locate actual callers before deleting an API. Start with the
affected Core, MVP or GPUI tests, then run:

```sh
scripts/check-gpui.sh
scripts/check.sh
```

Use controllable ordering tests for stale asynchronous results; a passing build
does not establish native focus, physical IME, process memory or GUI behavior.

Follow [code-style.md](docs/code-style.md) and
[development.md](docs/development.md). Do not hand-edit generated localization,
token, runtime-policy, file-type or feature-catalog outputs; update their
sources and run the documented generator. Generated Rust must stay in sync.

Package with `scripts/build-macos.sh`; it writes `build/Anywhere.app` through
`scripts/build-gpui.sh`. Physical IME, material, Quick Look, status-item,
hotkey, login-item and WebView evidence stays separate from build and unit-test
success.

Maintain one current explanation in the relevant topic guide. Keep
implementation maps in [architecture.md](docs/architecture.md), release
scope/evidence in the current MVP page, and dated observations in the archive.
State unimplemented or unverified behavior explicitly instead of copying old
build results forward.
