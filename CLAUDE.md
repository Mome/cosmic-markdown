# Cosmic Markdown — Project Instructions

A simple markdown viewer/editor for the COSMIC desktop, written in Rust with [libcosmic].

## Git workflow

Commit work along the way — don't leave large amounts of work uncommitted:

- Make a commit after each meaningful unit of work (e.g. each completed
  implementation phase from `docs/plan.md`, or a self-contained fix).
- Keep each commit building (`cargo build --locked`) and clippy-clean
  (`just check`).
- Update `docs/log.md` and `CHANGELOG.md` in the **same** commit as the change
  they describe.
- Write clear, imperative commit messages summarizing what changed and why.

## Documentation conventions

This project maintains three documentation artifacts. Keep them current as part of the work — do not treat them as an afterthought.

### `docs/specs.md` — Specifications

Holds the specifications for this project: what the application is, what it must do, and the constraints it operates under. When a feature's intended behavior changes, update the spec to match before or alongside the implementation. The spec describes *what* and *why*, not the decision trail (that lives in the log).

### `docs/log.md` — Decision log

Records every notable decision made for the project. Append new entries at the top (reverse chronological). Use this exact format for each entry:

```
## YYYY-MM-DD — <short title>

**Context:** What prompted the decision; the problem or question.
**Decision:** What was decided.
**Rationale:** Why this option was chosen over alternatives.
**Consequences:** Follow-up work, trade-offs, or things to watch.
```

One entry per decision. Do not rewrite past entries; if a decision is reversed, add a new entry that references the old one.

### `CHANGELOG.md` — Changelog (repo root)

Logs user-facing and notable repo changes. Follow the [Keep a Changelog](https://keepachangelog.com/) convention:

- Keep an `## [Unreleased]` section at the top for changes not yet released.
- Group entries under `### Added`, `### Changed`, `### Fixed`, `### Removed`, etc.
- On release, rename `[Unreleased]` to the version with the release date (`## [0.2.0] - YYYY-MM-DD`) and start a fresh `[Unreleased]`.
- Add an entry here whenever a change is significant enough that a user or packager would care.

## Build & tooling

A [justfile](./justfile) drives the project (uses [casey/just]):

- `just` / `just build-release` — build (release)
- `just run` — build and run
- `just check` — run clippy
- `just install` — install into the system

Localization uses [Fluent]; translation files live in [i18n/](./i18n).

[libcosmic]: https://github.com/pop-os/libcosmic
[casey/just]: https://github.com/casey/just
[Fluent]: https://projectfluent.org/
