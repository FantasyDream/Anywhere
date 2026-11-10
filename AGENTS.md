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
| Features | Domain behavior and statically registered first-party capabilities | `crates/anywhere-core/src/features/` |
| Persistence | SQLite, bounded source reads and stored preferences | `crates/anywhere-core/src/persistence/` |
| Platform | OS capabilities, application identity, paths and native operations | `crates/anywhere-platform/src/` |
| Bridge | Typed API and DTO conversion, without business policy | `crates/anywhere-bridge-flutter/src/api.rs` and `src/api/` |
| Flutter | Pages, input, selection, focus and window presentation | `apps/flutter/lib/launcher/`, `features/`, `tools/`, `settings/`, `ui/` |
| Native Host | Window lifecycle, hotkeys, text input, permissions and WebView hosting | `apps/flutter/macos/Runner/` |
| Web plugins | Plugin business logic through the shared SDK and theme | `packages/plugin-sdk/`, `plugins/examples/` |

First-party features are compiled modules with registration contributions, not
hot-loaded plugins. Third-party plugins use the existing package/grant/SDK boundary: WebView for UI,
on-demand QuickJS for finite background jobs, and the same Rust capability broker.
Third-party Context types must stay in `plugin.<package-id>.*`; use the existing
Context registry and never convert script data into native file authority. The legacy declarative protocol is compatibility code; do not extend
it into another UI framework.

## Find the behavior before editing

- Launcher navigation and retention: `launcher_controller.dart`,
  `launcher_view_state.dart`, `application/session_*` and
  [launcher-interaction.md](docs/launcher-interaction.md).
- Action ranking: `core/action/matcher.rs`, `core/ranking/` and
  `core/search/`. Empty action lists compare usage count, last use, relevance
  and stable IDs. Flutter only filters the chosen scope and limits visible rows.
  Keyword search, file results and source history lists have separate contracts.
- File search scanning: `features/filesystem/scan.rs` owns query lifetime; its
  `query`, `scope`, `record`, `traversal`, `progress` and `pagination` modules own
  matching, scope checks, record format, enumeration and result delivery.
  `result_store.rs` keeps continuous external sorting; see [file-search.md](docs/file-search.md).
- Installed application index: `application/application_index.rs` owns scan
  generations, full/filtered catalogs, exclusions and completion notifications;
  `session_runtime.rs` revalidates its immutable snapshot before publication.
- Activation Context reads: `application/context_capture.rs` owns source policy,
  frozen application reads and late capture publication; its `clipboard.rs` owns
  clipboard requests, age and fact publication. `activate.rs` joins these with Git
  and Usage refreshes without sharing the capture policy lock.
- Committed execution requests: `application/execution_request.rs` freezes the target,
  source and authorization before queueing; its `worker.rs` runs blocking dispatch,
  Web/process handoff and Usage recording. `execute.rs` maps results and menu recovery.
- Feature catalog publication: `application/feature_catalog.rs` owns definitions,
  providers, candidate sources, policy, catalog versions and startup binding claims;
  Application revokes Session / Web execution under the same publication locks.
- Application history: source packages in
  `crates/anywhere-core/resources/application-history/`; catalog and transforms
  in `application/application_history/`; state and authority in
  `features/application_history/`; object actions in
  `application/history_object_actions.rs`; UI in Flutter's matching feature.
  See [the source guide](docs/application-history-sources.md).
- Settings close decisions live in `settings/settings_controller.dart`; search drafts
  and queued edits in `settings/settings_edits.dart`. The Screen only renders choices.
  Authoritative Host saves and file authorization live in `settings/settings_save_coordinator.dart`;
  shortcut registration and feature mutation queues in `settings/shortcut_catalog_coordinator.dart`
  and `settings/feature_catalog_coordinator.dart`.
  Shared object-action request ordering lives in `tools/object_action_coordinator.dart`;
  sessions retain target validation and their own menu presentation.
- Native recording and Web channel ownership live in `ShortcutRecordingHost.swift`
  and `WebToolsHost.swift`; Launcher/Settings presentation generations and shared
  view movement live in `LauncherWindowCoordinator.swift` under `macos/Runner/`.
  Native settings, save effects and permission dispatch live in `SettingsHost.swift`;
  OS login-item adaptation lives in `LoginItemService.swift`.
- A new first-party capability should contribute through its registration,
  registry and tool-page interfaces instead of adding an application-specific
  branch to the Launcher.

Three kinds of history are intentionally separate: Launcher **Usage** stores
action counts; **application history** reads one selected external source;
**clipboard history** persists explicitly enabled local clipboard content.
Do not merge them into a global activity log.

## Preserve these contracts

- Scope filters actions; Context determines applicability. Current App
  aggregates available domains and includes menus without a separate menu Scope.
- Unavailable menus are hidden. Missing Accessibility permission removes menus
  without blocking other actions. Menu identity and legacy usage remain
  quarantined as described in [current-app-actions.md](docs/current-app-actions.md).
- Version checks belong immediately before async publication and execution.
  A stable action ID alone never authorizes an old candidate or changed target.
- Keep files, directories, workspace files and remote URIs distinct. Do not
  infer a workspace's first directory or choose an ambiguous application.
- Do not change the renderer to resolve resource issues; retain Impeller.

## Shared UI defaults

Pages inherit the application theme; do not create a nested MaterialApp or copy
colors, fonts and control shapes into each page. Flutter defaults live in
`ui/app_style.dart`, shared layouts in `ui/app_components.dart`, and tokens in
`resources/design-tokens.json`. Web documents receive `resources/web-controls.css`
automatically. Use ordinary widgets/HTML first and customize only actual layout
or semantic exceptions. Avoid generic page/form schemas and one-use wrappers.

## Verify and document

Use `rg` to locate actual callers before deleting an API. Start with the affected
Core/MVP or Flutter tests, then run `scripts/check.sh` for the final change.
Use controllable ordering tests for stale asynchronous results; a passing build
does not establish native focus, physical IME, process memory or GUI behavior.

Follow [code-style.md](docs/code-style.md) and
[development.md](docs/development.md). Do not hand-edit FRB, localization, token,
keymap or catalog generated files; update their sources and run the documented
generator. A Bridge API change requires regenerated bindings and a compiled
bridge crate.

Maintain one current explanation in the relevant topic guide. Keep implementation
maps in [architecture.md](docs/architecture.md), release scope/evidence in the
current MVP page, and dated observations in the archive. State unimplemented or
unverified behavior explicitly instead of copying old build results forward.
