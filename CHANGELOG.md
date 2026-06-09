# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Project documentation conventions: `docs/specs.md` (specifications),
  `docs/log.md` (decision log), and this changelog, described in `CLAUDE.md`.
- v1 specification and implementation plan (`docs/plan.md`).
- Single-document model with **Source** and **View** modes, toggled from the
  header bar. Source is editable Markdown (`text_editor`); View renders the live
  buffer with the built-in Markdown widget. A **File → New** action starts an
  empty document.
- File operations: **Open**, **Save**, and **Save As** via the xdg portal file
  dialogs, reading and writing UTF-8. The document's line-ending convention
  (LF/CRLF) is detected on open and preserved on save. File errors are shown in
  a dismissible banner.
- Unsaved-changes protection: New, Open, and closing the window prompt to
  **Save**, **Discard**, or **Cancel** when the document has unsaved edits.
- External-change detection: the open file is watched on disk (via `notify`).
  When it changes externally, the document reloads automatically if there are no
  unsaved edits, or prompts to **keep your changes** or **load from disk** on a
  conflict. The app's own saves are not treated as external changes.
- Keyboard shortcuts: New (`Ctrl+N`), Open (`Ctrl+O`), Save (`Ctrl+S`), and Save
  As (`Ctrl+Shift+S`), shown as accelerators in the File menu.
- An **Edit** menu with Cut, Copy, Paste, and Select All (active in Source mode).
  The editor also handles the standard `Ctrl+X/C/V/A` shortcuts directly.
- **Find** (`Ctrl+F`) and **Replace** (`Ctrl+H`): a find bar with match count and
  Previous/Next navigation, plus Replace and Replace All. All matches are
  highlighted in the editor. `Ctrl+F`/`Ctrl+H` toggle the bar closed when already
  showing that mode, and switch between find and replace otherwise; `Esc` closes
  it. Select All now shows its `Ctrl+A` shortcut in the Edit menu; Find/Replace
  show theirs too.
- Entering Source mode now focuses the editor so you can type right away.
- Zoom the content text with `Ctrl++` / `Ctrl+-` (reset with `Ctrl+0`) or the
  View menu. Zoom scales the editor and rendered view together, leaving the
  window chrome unchanged.
- **Undo** (`Ctrl+Z`) and **Redo** (`Ctrl+Shift+Z` / `Ctrl+Y`), implemented as an
  application-level history of buffer snapshots (the editor widget has no
  built-in undo). Runs of typing/deletion coalesce into single steps.
- Toggle the window header bar (decorations) with `Ctrl+Shift+H` or the View
  menu, for a distraction-free view.
- The Source/View toggle is now a single header icon button (eye to preview,
  pencil to edit) with a tooltip, a `Ctrl+E` shortcut, and a View-menu entry,
  replacing the two text buttons. Uses bundled Pop Icons (CC-BY-SA-4.0).

### Changed

- Unified the application ID to `dev.cosmic.CosmicMarkdown` across the code,
  justfile, and resources (previously inconsistent between `dev.mmurphy.Test`
  and `com.github.mome.cosmic-markdown`). Renamed the desktop and metainfo
  resource files to match the app ID.

### Fixed

- "Open with" / command-line file opening now works: the file path passed by
  the desktop entry (`Exec=... %F`) is read and opened on startup (previously it
  was ignored, opening an empty window). `just install` now also runs
  `update-desktop-database` so the app registers for `text/markdown` files.
- Content margins: removed the scrollbar/content gap in the rendered View that
  made the right margin look larger than the others, and reduced the top margin
  when the header bar is shown (it was stacking with the header's own
  separation).
- The Source editor now renders on the primary-container surface with rounded
  corners and an accent focus border, so it stands out from the window
  background instead of blending in (libcosmic's default `text_editor` style
  uses the plain window color). The rendered View uses the same surface.
- Menus now set an explicit item width/height, fixing the empty gray box that
  appeared because the dropdown items were unsized.
- AppStream metainfo: added a description, developer, homepage/bugtracker URLs,
  Markdown/editor categories and keywords, and a 0.1.0 release entry; corrected
  the invalid `<binaries>` provides wrapper. Removed an unused localization
  string. Validates clean with `appstreamcli`.
- `just install`/`uninstall` now reference the correctly named resource files
  and install the icon under the app-ID name, so the desktop entry's `Icon`
  resolves. Advertised `text/markdown` and `text/plain` MIME types for
  "Open with" support.
