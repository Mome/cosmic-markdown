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
