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

### Changed

- Unified the application ID to `dev.cosmic.CosmicMarkdown` across the code,
  justfile, and resources (previously inconsistent between `dev.mmurphy.Test`
  and `com.github.mome.cosmic-markdown`). Renamed the desktop and metainfo
  resource files to match the app ID.

### Fixed

- `just install`/`uninstall` now reference the correctly named resource files
  and install the icon under the app-ID name, so the desktop entry's `Icon`
  resolves. Advertised `text/markdown` and `text/plain` MIME types for
  "Open with" support.
