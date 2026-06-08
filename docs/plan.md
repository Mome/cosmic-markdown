# Cosmic Markdown — v1 Implementation Plan

Derived from [specs.md](./specs.md) and the decisions in [log.md](./log.md). Scope
is **v1**: Source + View modes, single document, New/Open/Save(+Save As),
unsaved-changes prompts, UTF-8, open-with/CLI, and live external-change
detection. WYSIWYG **Edit** mode, code highlighting, and image rendering are out
of scope (v2).

## Target architecture

A single libcosmic `Application` (`AppModel`) holding one open document. The
template's nav-bar/multi-page demo is removed; the window body shows either the
Source editor or the rendered View depending on the active mode.

```
AppModel
├── core, config, key_binds, about        (kept from template)
├── document: Document                     (path, buffer, dirty, mode)
├── markdown: markdown::Content            (parsed View content, rebuilt from buffer)
├── dialog: Option<DialogState>            (unsaved-changes / conflict prompts)
└── file watch handle + suppress-self flag (notify)
```

```
Document
├── path: Option<PathBuf>
├── editor content: widget::text_editor::Content
├── dirty: bool
├── mode: Mode { Source, View }
└── disk_mtime: Option<SystemTime>         (for self-write suppression)
```

## Dependencies

- Add **`notify`** (file watching). Wrap it in an iced `Subscription` via
  `cosmic::iced::stream::channel` (same pattern as the template's timer
  subscription) so disk events arrive as `Message`s.
- `tokio` (already present) covers async file IO if needed.
- Do **not** enable the `markdown`/`highlighter` highlighter feature yet — v1
  code blocks are plain. The `markdown` widget itself is already available.

## Phase 0 — App identity cleanup (prerequisite)

The scaffold has an inconsistent ID (`dev.mmurphy.Test` in code,
`com.github.mome.cosmic-markdown` in resources/justfile). Unify everything to
**`dev.cosmic.CosmicMarkdown`**:

- [src/app.rs](../src/app.rs) — `APP_ID`.
- [justfile](../justfile) — `appid`.
- [resources/app.desktop](../resources/app.desktop) — `Icon`, and rename file to
  `dev.cosmic.CosmicMarkdown.desktop`.
- [resources/app.metainfo.xml](../resources/app.metainfo.xml) — `<id>`,
  `<launchable>`, and rename to `dev.cosmic.CosmicMarkdown.metainfo.xml`.
- Confirm the `MimeType` in the desktop file advertises `text/markdown` (and
  `text/plain`) so "Open with" works from file managers.

## Phase 1 — Strip the template demo

In [src/app.rs](../src/app.rs):

- Remove `nav_bar` model, `Page` enum, `on_nav_select`, and the three demo
  pages; drop `nav_model()` (return nothing) so there is no side nav.
- Remove the `time` / `watch_is_active` counter demo and `WatchTick`/`ToggleWatch`.
- Keep: `core`, `config`, `about`, `key_binds`, context drawer (About), and the
  config-watch subscription.
- Replace [src/config.rs](../src/config.rs) `demo` field with real settings as
  they appear (e.g. default mode); keep it minimal for now.

## Phase 2 — Document model & state

- Define `Mode { Source, View }` and a `Document` struct as above.
- Add `document` and `markdown: markdown::Content` to `AppModel`.
- `view()` renders by mode:
  - **Source**: `widget::text_editor` bound to the document content, full size.
  - **View**: `widget::markdown::view(&self.markdown, ...)` inside a
    `scrollable`.
- On every edit in Source, mark `dirty = true` and rebuild `markdown::Content`
  from the buffer (so switching to View is live). For larger docs this can be
  debounced later; v1 can rebuild on each edit or on mode-switch.
- Window title reflects the file name + a dirty marker (e.g. `*name.md`).

## Phase 3 — Modes & menu/header UI

- Add a mode toggle to the header bar (e.g. a segmented button or two toggle
  buttons: Source / View). Edit is not shown in v1 (or shown disabled — TBD,
  default: not shown).
- Build a **File** menu (`menu::bar`) with New, Open, Save, Save As, plus the
  existing About under View/help.
- Wire key binds: New `Ctrl+N`, Open `Ctrl+O`, Save `Ctrl+S`, Save As
  `Ctrl+Shift+S`.
- Default mode: **Source** for new documents, **View** for opened files.

## Phase 4 — File operations

- **Open/Save As dialogs**: use the xdg portal file chooser (libcosmic dialog /
  `ashpd`), filtered to Markdown/text. Run as async `Task`s returning a
  `Message` with the chosen path.
- **New**: clear document (path `None`, empty buffer, mode Source), guarded by
  the unsaved-changes check.
- **Open(path)**: read UTF-8, load into the editor content, set path, clear
  dirty, default to View, (re)arm the file watch on the new path.
- **Save / Save As**: write UTF-8, preserving existing line endings (default
  `\n` for new files); clear dirty; record `disk_mtime`; arm watch if newly
  pathed.
- **Encoding**: read/write UTF-8 only; on non-UTF-8 input, show an error dialog
  rather than corrupting.

## Phase 5 — Unsaved-changes handling

- Add `DialogState` for a save/discard/cancel prompt.
- Intercept **New**, **Open**, and **window close (Quit)** when `dirty`:
  - Show the prompt; on Save → save then continue the pending action; on Discard
    → continue; on Cancel → abort.
- For Quit, hook the app-exit path so the prompt can veto closing.

## Phase 6 — External change detection (`notify`)

- Spawn a `notify` watcher on the open file's path; bridge events into a
  `Message::FileChangedOnDisk` via a `Subscription` channel. Re-arm on
  Open/Save As; disarm when no file is open.
- **Self-write suppression**: when the app writes the file, record the new
  `disk_mtime` (and/or set a short-lived "ignore next event" flag) so the app's
  own save is not treated as an external change.
- On a genuine external change:
  - if `!dirty` → reload the buffer from disk automatically (refresh View);
  - if `dirty` → show a conflict prompt: **Keep mine** (ignore disk) or **Load
    from disk** (discard local edits).

## Phase 7 — Localization

- Replace demo strings in [i18n/en](../i18n/en) with real ones: menu labels
  (New/Open/Save/Save As), mode labels (Source/View), dialog text
  (unsaved-changes, conflict, encoding error), title/dirty marker.
- Remove `welcome` / `page-id` demo keys.

## Phase 8 — Packaging & docs

- Update [resources/app.metainfo.xml](../resources/app.metainfo.xml) summary,
  description, screenshots, and release entries.
- Verify `just build-release`, `just run`, `just check` (clippy clean), and
  `just install` with the new app ID.
- Update [CHANGELOG.md](../CHANGELOG.md) under `[Unreleased]` as features land.

## Open implementation details (decide during build, not blockers)

- Exact mode-switch UI widget (segmented control vs. toggle buttons).
- Whether to debounce View re-parsing on large files.
- Whether Quit-veto is cleanly supported by the current libcosmic version (may
  require handling the close request in `update`).
- Markdown vs. plain-text file filter specifics in the portal dialog.

## Suggested build order

Phase 0 → 1 (get a clean, building shell) → 2 → 3 (Source/View visible and
toggleable on an in-memory doc) → 4 (real files) → 5 (safety) → 6 (watching) →
7 → 8. Each phase should leave the app building and runnable.
