# Decision Log

Notable decisions made for Cosmic Markdown. Newest entries first.

Format for each entry:

```
## YYYY-MM-DD — <short title>

**Context:** What prompted the decision; the problem or question.
**Decision:** What was decided.
**Rationale:** Why this option was chosen over alternatives.
**Consequences:** Follow-up work, trade-offs, or things to watch.
```

---

## 2026-06-08 — Scope Markdown support to the built-in widget

**Context:** The spec required Markdown features (notably definition lists, footnotes, autolinks) beyond what libcosmic's built-in `markdown` widget supports. Source review (libcosmic `f0f6893`, vendoring iced's `markdown` widget) confirmed the widget renders only a closed set of `Item` variants and enables only the `TABLES`, `STRIKETHROUGH`, and `TASKLISTS` pulldown-cmark options.
**Decision:** Limit Markdown requirements to what the built-in widget can render: CommonMark base, GFM tables/task lists/strikethrough, code highlighting (via the `highlighter`/syntect feature), and images (via a custom `Viewer`, since the default only shows alt text). Drop definition lists, footnotes, autolinks, and raw/inline HTML as explicit non-features.
**Rationale:** Reusing the built-in widget keeps the implementation simple (the project's guiding principle) and avoids adopting or building a custom renderer (e.g. Frostmark, as Cedilla did). The dropped features are not essential to the core use case.
**Consequences:** Code highlighting colors will follow syntect themes, not the COSMIC theme. Image rendering needs a custom `Viewer` impl. If definition lists/footnotes/HTML become required later, the rendering stack would need to be reconsidered (Frostmark or custom). See the earlier note to revisit Frostmark.

## 2026-06-09 — Icon toggle button for Source/View

**Context:** Replace the two Source/View text buttons with a single icon toggle button plus a hotkey, modelled on Cedilla's eye-icon preview toggle.
**Decision:** Header now shows one `button::icon` toggle: an eye (`show-symbolic`) in Source mode (→ preview) and a pencil (`edit-symbolic`) in View mode (→ edit), with a tooltip. Added `Message::ToggleMode` (replacing `SetMode(Mode)`), a `Ctrl+E` key binding, and a "Toggle Preview" item in the View menu. Icons are bundled from Cedilla's set — **Pop Icons by System76, CC-BY-SA-4.0** — copied to `resources/icons/bundled/` with a `COPYING` attribution; embedded via `include_bytes!` so no install step is needed.
**Rationale:** A single icon toggle is more compact and matches COSMIC conventions (cosmic-edit/Cedilla use eye icons). Pop Icons are CC-BY-SA-4.0, compatible with our MPL-2.0 code given attribution. `Ctrl+E` doesn't collide with the editor's built-in bindings.
**Consequences:** Builds clean and pedantic-clippy-clean. `mode-source`/`mode-view` strings removed; `show-preview`/`edit-source`/`toggle-preview` added.

## 2026-06-09 — Defer menu hover-highlight fix

**Context:** Menu items don't highlight on hover in our app (and not in Cedilla either), but they do in COSMIC Text Editor. Investigated the cause.
**Decision:** Defer the fix and keep the menu code in its natural state (default `menu::bar`, no forced highlighting). Reverted the earlier `PathHighlight::Full` change and an experimental fix that enabled the `wayland` + `surface-message` libcosmic features and wired `on_surface_action`/`window_id` to render menus as popup surfaces.
**Rationale:** Findings: libcosmic's default features include `winit` and `multi-window` but **not** `wayland` or `surface-message`. The proper COSMIC popup-menu path is `#[cfg(all(multi-window, wayland, target_os="linux", winit, surface-message))]`; without `wayland`+`surface-message` the menu falls back to an in-window overlay whose hover highlight doesn't work on Wayland. cosmic-edit enables `wayland`, so it gets popup menus that highlight; template-based apps (us, Cedilla) don't. The experimental fix (enable those features + wire `on_surface_action`/`window_id_maybe`) compiled cleanly and pulled in the Wayland stack (cctk/smithay/cosmic-protocols), but was not runtime-verified, so the user chose to defer rather than bank a large unverified change.
**Consequences:** Menu hover remains unhighlighted for now. When revisited: enable `wayland` + `surface-message` features and set `.on_surface_action(Message::Surface).window_id_maybe(self.core.main_window_id())` on the menu bar (with a `Message::Surface(cosmic::surface::Action)` handler dispatching `cosmic::Action::Cosmic(cosmic::app::Action::Surface(_))`). Verify on a real Wayland session before committing.

## 2026-06-09 — Edit menu, menu sizing fix, and View surface

**Context:** User feedback after a live run: the View should match the editor surface; a stray gray box appeared in menus; an Edit menu with common actions was wanted.
**Decision:** (1) Wrapped the rendered View in a container styled identically to the editor (primary-container surface, `radius_s`, divider border) via a shared `surface_style`. (2) Set `item_height`/`item_width`/`spacing` on `menu::bar` — the dropdown was rendering as an unsized gray box without them (the menu example sets these). (3) Added an Edit menu with Cut/Copy/Paste/Select All, operating on the `text_editor` buffer (selection via `Content::selection`, clipboard via `iced::clipboard::read`/`write`, paste through `Message::Pasted`); items are `ButtonDisabled` outside Source mode.
**Rationale:** The editor already handles `Ctrl+X/C/V/A` natively when focused, so those keys are intentionally NOT added to the global `key_binds` (avoids double-firing); the Edit menu provides clickable equivalents without accelerator labels. Undo/Redo were omitted because the cosmic-text editor widget exposes no undo action — showing dead/disabled items would be misleading.
**Consequences:** Builds clean and pedantic-clippy-clean. Undo/redo remain unavailable until the underlying widget supports them. Still not runtime-verified beyond the user's manual check.

## 2026-06-08 — Keyboard shortcuts added to v1

**Context:** Keyboard accelerators were deferred during Phases 3–8; the user asked to include them in v1.
**Decision:** Populated the `key_binds` map (`Ctrl+N` New, `Ctrl+O` Open, `Ctrl+S` Save, `Ctrl+Shift+S` Save As) and added a `keyboard::listen()` subscription that maps `KeyPressed` events to `Message::Key`. The handler matches the event against `key_binds` via `KeyBind::matches` (using the base `key` plus `physical_key` fallback) and dispatches the bound action's message. Populating `key_binds` also makes the accelerators display in the File menu.
**Rationale:** libcosmic does not auto-fire menu `key_binds`; the app must listen for key events and match them itself (the documented pattern; `iced` exposes only `keyboard::listen()`, not `on_key_press`). Matching on the base `key` keeps `Ctrl+Shift+S` working regardless of shift-applied character case.
**Consequences:** Builds clean and pedantic-clippy-clean. v1 now includes keyboard shortcuts. Not runtime-verified (no display).

## 2026-06-08 — Phases 7 & 8 complete: i18n & packaging

**Context:** Final polish phases (per `plan.md`) — review localization strings and the AppStream/packaging metadata.
**Decision:** Audited `fl!` usage against the `en` Fluent file (all used keys defined; removed the unused `git-description` template leftover). Expanded `dev.cosmic.CosmicMarkdown.metainfo.xml` with a description and feature list, developer tag, homepage/bugtracker URLs, Utility/TextEditor/Markdown categories and keywords, and a 0.1.0 release entry; fixed the invalid `<binaries>` provides wrapper. Verified the desktop file (`desktop-file-validate`) and metainfo (`appstreamcli validate`) both pass.
**Rationale:** Clean metadata is required for store/Flathub listing and correct desktop integration.
**Consequences:** v1 is feature-complete with clean packaging metadata. Still outstanding and explicitly deferred: keyboard accelerators (Ctrl+N/O/S), a screenshot in the metainfo (pedantic note), and the v2 items (code highlighting, image rendering). Not yet runtime-verified (no display in the build environment).

## 2026-06-08 — Phase 6 complete: external-change detection

**Context:** Phase 6 (per `plan.md`) — detect when the open file is modified by another application.
**Decision:** Added the `notify` dependency and a `Subscription::run_with(path, …)` file watch, keyed by path so it re-arms on Open/Save As and stops when no file is open. The watcher bridges notify's background-thread events into the async stream via an unbounded channel and emits `FileChangedOnDisk`. On that event the file is re-read; **self-write suppression** is done by comparing the on-disk contents to the buffer (newline-normalized) — equal means our own save or a no-op and is ignored. Otherwise: auto-reload when clean, or a `ConflictReload` dialog (Keep my changes / Load from disk) when dirty.
**Rationale:** Content comparison is simpler and more robust than mtime bookkeeping and naturally ignores self-writes and no-op touches. The stream is boxed (`Pin<Box<dyn Stream>>`) to avoid the edition-2024 RPIT lifetime capture that would otherwise break the `fn(&D) -> S` builder bound.
**Consequences:** Builds clean and pedantic-clippy-clean. Known limitation: the watch follows the file directly (inotify), so editors that save via atomic rename-replace may not be tracked after the first replace; watching the parent directory could be added later if needed. v1 feature work is now complete; remaining: Phase 7 (i18n pass), Phase 8 (packaging), and keyboard accelerators.

## 2026-06-08 — Phase 5 complete: unsaved-changes prompts

**Context:** Phase 5 (per `plan.md`) — guard New, Open, and Quit against discarding unsaved edits.
**Decision:** Added a modal confirm dialog (`Application::dialog()` + `widget::dialog()`) with Save/Discard/Cancel. New and Open route through `guard_or_perform`, which shows the dialog when `dirty` and otherwise runs the action via `perform_pending`. Window close is intercepted with `on_close_requested` (returns `RequestQuit` to veto when dirty); a `quitting` flag lets the programmatic `window::close` proceed without re-prompting. Save-then-continue is handled by stashing the deferred action in `pending` and resuming it in the `FileSaved` handler; a cancelled/failed save clears `pending`.
**Rationale:** `on_close_requested` returning `Some` is libcosmic's documented way to override window closing. Threading the deferred action through `pending` keeps the async save flow simple without nested dialogs.
**Consequences:** Builds clean and clippy-clean. Quit-veto and dialog modality were validated by code/flow review, not runtime (needs a display). Remaining: Phase 6 (external-change watch), Phase 7 (i18n pass), Phase 8 (packaging), and the deferred keyboard accelerators.

## 2026-06-08 — Phase 4 complete: file operations

**Context:** Phase 4 (per `plan.md`) — wire Open/Save/Save As, UTF-8 IO, and line-ending preservation.
**Decision:** Added Open/Save/Save As to the File menu, driven by async tasks (`cosmic::task::future`) using the xdg portal dialogs (`file_chooser::open`/`save::Dialog`). Open reads UTF-8 via `tokio::fs::read_to_string` and switches to View; Save writes to the known path or falls back to Save As; both write UTF-8 via `tokio::fs::write`. Added a `LineEnding` (LF/CRLF) detected on open and re-applied on save. File errors surface in a dismissible `widget::warning` banner; cancelled dialogs are no-ops.
**Rationale:** Mirrors libcosmic's `open-dialog` example. Used ashpd's `FileFilter` (`.glob`) rather than the rfd `.extension` API, since `xdg-portal` is the enabled feature. Keyboard accelerators were deferred (menu items are clickable) to keep the phase focused.
**Consequences:** Builds clean and clippy-clean. The `dirty` flag is now maintained but not yet acted upon — Phase 5 adds the unsaved-changes prompts (New/Open/Quit) and Phase 6 the external-change watch. Keyboard accelerators (Ctrl+N/O/S) still pending.

## 2026-06-08 — Phase 2+3 complete: document model, Source/View modes

**Context:** Phases 2 and 3 (per `plan.md`) — add the document/state model and make Source/View visible and toggleable on an in-memory document.
**Decision:** Added `Document` (path, `text_editor::Content` buffer, dirty flag, mode) and a `Mode { Source, View }` enum to `AppModel`, plus a `markdown::Content` rebuilt from the buffer. `view()` renders the `text_editor` in Source mode and `markdown::view` inside a scrollable in View mode. Mode toggle lives in `header_end` as two text buttons (active = `Button::Suggested`). Added a File menu with New. Markdown render settings derive from the active COSMIC theme via `cosmic::theme::is_dark()` mapped to iced `Theme::Dark/Light` (the markdown widget's `Settings: From<&iced::Theme>`; `cosmic::theme::Theme` implements the widget's `Catalog`). Replaced demo i18n keys with real UI strings.
**Rationale:** Confirmed against libcosmic source that `cosmic::theme::Theme` implements `markdown::Catalog`, so the built-in widget works directly; the iced `markdown` example mirrors this Source+View design. No source-syntax highlighting in v1 (kept plain, per spec).
**Consequences:** Builds clean and clippy-clean. GUI not yet runtime-verified (needs a display session). Open/Save/Save As, key-bind accelerators, unsaved-changes guarding, and the file watch remain for Phases 4–6.

## 2026-06-08 — Phase 1 complete: stripped template demo

**Context:** Phase 1 (per `plan.md`) — remove the cosmic-app-template demo scaffolding to leave a clean single-window shell.
**Decision:** Removed the nav bar (`nav_model`/`on_nav_select`), the `Page` enum and three demo pages, and the counter demo (`time`/`watch_is_active`, `WatchTick`/`ToggleWatch`) with its timer subscription. Kept `core`, `config`, `about`, `key_binds`, the About context drawer, the View menu, and the config-watch subscription. `view()` is now a centered placeholder pending the document area. Demo i18n keys (`welcome`, `page-id`) left in place for Phase 7 cleanup.
**Rationale:** Establishes a minimal, building baseline before adding document/state logic.
**Consequences:** `cargo build --locked` and `cargo clippy` are clean. Ready for Phase 2 (document model & state).

## 2026-06-08 — Phase 0 complete: unified app identity

**Context:** Implementation Phase 0 (per `plan.md`) — the scaffold had an inconsistent app ID and a resource-file naming mismatch that would break `just install`.
**Decision:** Set the app ID to `dev.cosmic.CosmicMarkdown` in `src/app.rs`, `justfile`, and both resource files; renamed `resources/app.desktop` → `dev.cosmic.CosmicMarkdown.desktop` and `resources/app.metainfo.xml` → `dev.cosmic.CosmicMarkdown.metainfo.xml` to match the justfile's expectations. Fixed the icon install to use the app-ID filename, added the appdata file to uninstall, and set the desktop `MimeType`/categories/keywords.
**Rationale:** A single consistent ID is required for correct desktop integration, single-instance behavior, and config storage; the rename also fixes a latent install bug from the template.
**Consequences:** `cargo build --locked` succeeds. Ready to proceed to Phase 1 (strip the template demo).

## 2026-06-08 — Draft v1 implementation plan

**Context:** All v1 product decisions were settled; the work needed a concrete, phased build plan.
**Decision:** Recorded a phased implementation plan in [plan.md](./plan.md) (Phase 0 identity cleanup → strip demo → document model → modes/UI → file ops → unsaved-changes → external-change watch → i18n → packaging). Noted during review that the scaffold's app ID is inconsistent — `dev.mmurphy.Test` in `src/app.rs` vs. `com.github.mome.cosmic-markdown` in the justfile/resources — and must be unified to the chosen `dev.cosmic.CosmicMarkdown` as a prerequisite (Phase 0).
**Rationale:** A phase-ordered plan that keeps the app building at each step reduces risk and matches the "keep it simple" principle.
**Consequences:** Adds a `notify` dependency. Phase 0 touches `src/app.rs`, `justfile`, and both files in `resources/` (including renames).

## 2026-06-08 — Detect external changes to the open file

**Context:** Markdown files are often edited by other tools; the app should not silently show stale content or clobber external edits.
**Decision:** Watch the open file live with the `notify` crate (inotify on Linux). On external change: auto-reload if there are no unsaved local edits; if there are local edits (a conflict), prompt the user to keep their version or load the disk version. The app suppresses watcher events caused by its own saves, and re-points the watch when the open path changes (Open / Save As).
**Rationale:** Auto-reload when clean keeps the view fresh with zero friction; prompting only on genuine conflicts avoids data loss. Live watching is chosen over check-on-focus for immediacy, accepting the added `notify` dependency.
**Consequences:** Adds a `notify` dependency and a file-watch subscription to the architecture. Care is needed to debounce/ignore self-induced write events. This applies to v1.

**Context:** With the render stack and Markdown scope settled, a handful of decisions still blocked an implementation plan: app identity, source editor, document/state model, unsaved-changes handling, mode-switch semantics, encoding, and open-with behavior.
**Decision:**
- **App ID** `dev.cosmic.CosmicMarkdown`, display name "Cosmic Markdown".
- **Source editor**: libcosmic `text_editor` (cosmic-text), plain text, no source syntax highlighting in v1.
- **Document model**: single in-memory document — optional path, text buffer, dirty flag, active mode; opening a file replaces the current document in place.
- **Unsaved changes**: prompt to save/discard/cancel on New, Open, and Quit when dirty.
- **Mode-switch**: View always renders from the live Source buffer; default mode is View for opened files and Source for new documents.
- **Encoding**: UTF-8 only; preserve existing line endings, default `\n` for new files.
- **Open with / CLI**: a file-path argument opens that file, respecting single-instance (focus existing instance and open the file).
**Rationale:** Each choice favors the simplest behavior that meets the core use case, consistent with the project's guiding principle. The `dev.cosmic.*` namespace was chosen over GitHub-based reverse-DNS as it is not tied to a specific account.
**Consequences:** The v1 specification is now complete enough to produce an implementation plan. No further product decisions are outstanding for v1.

**Context:** Code syntax highlighting and image rendering are both supported in principle but are the two most involved pieces (highlighting pulls in syntect via the `highlighter` feature; images need a custom `Viewer` that loads files from disk). The guiding principle is simplicity.
**Decision:** Defer both to v2. In v1, code blocks render as plain monospace text and images render as their alt text only.
**Rationale:** Keeps the v1 surface minimal and avoids the extra dependency (syntect) and async image-loading work for the first release. Both can be layered on later without changing the architecture.
**Consequences:** v1 ships with no code coloring and no drawn images. The `highlighter` feature need not be enabled until v2.

## 2026-06-08 — Define initial product specification

**Context:** `docs/specs.md` was an empty scaffold; the application's scope and behavior needed to be pinned down.
**Decision:** The app is an equal viewer + editor with **three toggleable modes**: **Source** (raw Markdown text editing), **Edit** (WYSIWYG, editing the rendered document), and **View** (read-only rendered output). The WYSIWYG Edit mode is deferred to a later phase; the initial release ships Source and View only. File ops are New / Open / Save via xdg portals, single document at a time. Markdown support: CommonMark base, GFM extensions, definition lists, code highlighting, and images. Non-goals: export, cloud/sync, and real-time collaboration.
**Rationale:** Source + View cover the core viewer/editor need with low complexity, so they ship first. WYSIWYG editing is the hardest piece and is sequenced last to de-risk the initial release. Single-document scope avoids tab-management complexity early on. The feature set covers common Markdown needs without over-scoping.
**Consequences:** WYSIWYG Edit mode, multi-tab support, and export remain future work and are deliberately deferred. The chosen Markdown feature set drives renderer/parser requirements.

## 2026-06-08 — Scaffold project from cosmic-app-template

**Context:** A starting point was needed for a native COSMIC application.
**Decision:** Generate the project with `cargo generate gh:pop-os/cosmic-app-template`.
**Rationale:** The official template provides the standard COSMIC app structure — libcosmic wiring, i18n/Fluent setup, justfile build recipes, and resource layout — so we don't hand-roll boilerplate.
**Consequences:** Project layout, `justfile`, `i18n/` setup, and dependency choices follow template conventions; upstream template changes are not auto-merged and must be ported manually if desired.

## 2026-06-08 — Establish project documentation conventions

**Context:** The project lacked a defined home for specifications, decisions, and a user-facing change history.
**Decision:** Adopt three artifacts — `docs/specs.md` for specifications, `docs/log.md` for a decision log in this format, and a root `CHANGELOG.md` following Keep a Changelog. Conventions are recorded in `CLAUDE.md`.
**Rationale:** Separating *what/why* (specs), *decision trail* (log), and *change history* (changelog) keeps each document focused and easy to maintain.
**Consequences:** Future work should update the relevant document(s) as part of the change.
