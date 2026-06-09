# Cosmic Markdown — Specifications

A simple markdown viewer/editor for the COSMIC desktop.

## Overview

Cosmic Markdown is a native [COSMIC](https://github.com/pop-os/cosmic-epoch)
application built with [libcosmic](https://github.com/pop-os/libcosmic) for
viewing and editing Markdown documents. It was scaffolded from the
[cosmic-app-template](https://github.com/pop-os/cosmic-app-template) and follows
its structure and conventions.

- **Application ID:** `dev.cosmic.CosmicMarkdown`
- **Display name:** Cosmic Markdown

Viewing and editing are both first-class: the application is equally a reader
for rendered Markdown and an editor for Markdown source.

**Guiding principle: simplicity.** Cosmic Markdown intentionally stays
lightweight and focused. Where a feature would add significant complexity, it is
deferred or left out. A more full-featured COSMIC markdown editor already exists
([Cedilla](https://github.com/mariinkys/cedilla)); this project deliberately
aims to be the simpler alternative.

## Goals

- View Markdown documents as rendered output.
- Edit Markdown source.
- Integrate natively with the COSMIC desktop (theming, single-instance, xdg portals, localization).

## User interface

The window presents one of three modes at a time; the user toggles between them:

- **Source** — the raw Markdown source for direct text editing.
- **Edit** — a WYSIWYG editor that edits the rendered document directly.
  *(Deferred: this is the most complex mode and will be added in a later phase.
  The initial release ships Source and View only.)*
- **View** — read-only rendered output.

The user switches between Source and View with a single toggle button in the
header bar (an eye icon to preview, a pencil icon to edit) or the `Ctrl+E`
shortcut. The **View** is always rendered from the live Source buffer, so edits
made in Source are reflected immediately on switching to View. The default mode
is **View** when opening an existing file, and **Source** when creating a new
document.

The **Source** editor uses libcosmic's `text_editor` (cosmic-text) widget as a
plain-text editor. The Markdown source itself is not syntax-highlighted in v1.
An **Edit** menu provides Cut, Copy, Paste, Select All (`Ctrl+A`), Find
(`Ctrl+F`), and Replace (`Ctrl+H`) — all active in Source mode; the editor also
handles the standard `Ctrl+X/C/V/A` shortcuts directly when focused. Undo/redo
are not available (unsupported by the editor widget).

**Find / Replace.** A find bar (toggled by `Ctrl+F`, or `Ctrl+H` with a replace
row) searches the buffer for plain-text matches, shows the match count, and
navigates between matches (Previous/Next), selecting each in the editor. Replace
substitutes the current match; Replace All substitutes every occurrence.

Both the Source editor and the rendered View are presented on a distinct
"content" surface so they stand out from the window background.

Native COSMIC theming and window chrome are used throughout.

## Document model & file operations

A single document is open at a time (no multi-tab interface in this phase). The
document is held in memory as:

- an optional file path (`None` for an unsaved new document),
- the current text buffer,
- a "dirty" flag tracking unsaved changes,
- the active mode (Source / View).

Opening a file replaces the current document in place.

Operations (with keyboard shortcuts):

- **New** (`Ctrl+N`) — create a new, empty document (starts in Source mode).
- **Open** (`Ctrl+O`) — open an existing Markdown file via the xdg portal file
  dialog.
- **Save** (`Ctrl+S`) — write the current document to disk; **Save As**
  (`Ctrl+Shift+S`) for new/untitled documents.

**Unsaved changes.** When the document is dirty, New, Open, and Quit prompt the
user to save, discard, or cancel before proceeding.

**Encoding.** Files are read and written as UTF-8. Existing line endings are
preserved on save; new documents use `\n`.

**Open with / CLI.** Launching the application with a file path argument (e.g.
via the file manager's "Open with") opens that file. This respects the
single-instance behavior: an already-running instance is focused and opens the
file rather than starting a second process.

**External change detection.** While a file is open, the application watches it
on disk (live file watching via the `notify` crate / inotify on Linux) and
reacts when another application modifies it:

- If there are **no unsaved local edits**, the document is **reloaded
  automatically** from disk.
- If there **are unsaved local edits** (a conflict), the user is **prompted** to
  either keep their in-memory version or discard it and load the version from
  disk.

The application must ignore the change events caused by its own saves (e.g. by
suppressing watcher events around a write), so that saving does not trigger a
spurious "changed on disk" reaction. The watch follows the currently open file:
it stops watching the old path and starts watching the new one on Open / Save
As.

## Markdown support

Markdown support is **scoped to what the built-in libcosmic/iced `markdown`
widget can render**, to keep the implementation simple. The widget parses with
`pulldown-cmark` into a fixed set of item types; features outside that set are
deliberately not supported.

Supported in v1:

- **CommonMark base** — headings, paragraphs, emphasis (bold/italic), lists,
  links, inline code, fenced/indented code blocks, blockquotes, horizontal
  rules.
- **GFM subset** — tables, task lists, and strikethrough.

Code blocks render as plain monospace text in v1 (no syntax coloring), and
images render as their alt text in v1 (not drawn).

Deferred to v2:

- **Code highlighting** — syntax highlighting within fenced code blocks, via the
  widget's `highlighter` feature (syntect-based). Note: highlight colors follow
  syntect's themes rather than the COSMIC desktop theme.
- **Image rendering** — actually drawing images in the View. The widget does not
  draw images by default (it shows alt text only), so this requires a custom
  `Viewer` implementation that loads images from the document's location.

Explicitly **not** supported (the widget cannot render these):

- Definition lists.
- Footnotes.
- Autolinks (bare URLs are not auto-linked).
- Raw/inline HTML (e.g. `<br>`, `<details>`) — ignored.

## Localization

The UI is localized via [Fluent](https://projectfluent.org/); translation files
live in [i18n/](../i18n). New languages are added by copying the English (`en`)
locale and translating each message identifier.

## Non-goals

The following are explicitly out of scope for the current phase:

- **No WYSIWYG Edit mode in the initial release** — the Edit mode is planned but
  deferred to a later phase due to its complexity. Source and View ship first.
- **No export** to PDF, HTML, or other formats.
- **No cloud storage or sync** — local files only.
- **No real-time collaboration** — single-user editing only.
