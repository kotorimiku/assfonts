# assfonts Workspace Instructions

## Scope

- This repository is a Rust CLI for ASS subtitle font indexing, matching, subsetting, and embedding.
- Prefer small, surgical changes. Keep module boundaries intact instead of moving logic across files unless the task requires it.
- Do not edit generated or local-output directories such as `target/` and `out/`.

## Commands

- Build: `cargo build`
- Check: `cargo check`
- Clippy: `cargo clippy`
- Test: `cargo test`
- Show CLI help: `cargo run -- -h`
- Normal CLI processing uses the flattened `RunOptions` on the root command (no subcommand):
  ```bash
  cargo run -- -i examples/sample.ass -o out -f C:/Windows/Fonts
  ```
- Font index build uses the explicit `build` subcommand:
  ```bash
  cargo run -- build -f C:/Windows/Fonts -d ./.db
  ```

## Architecture

- `src/main.rs` wires clap parsing to two entry paths: `build` subcommand and the default processing flow.
- `src/cli.rs` defines the CLI surface. **There is no `run` subcommand**; normal processing is the default command path.
- `src/commands.rs` is the orchestration layer for indexing, per-file processing, reporting, cache usage, and validation.
- `src/ass.rs` parses ASS styles, dialogue overrides, and collects font requests with bold/italic state.
- `src/font.rs` discovers fonts, normalizes names, matches requests, and extracts TTC/OTC faces as standalone sfnt.
- `src/subset.rs` performs font subsetting via `allsorts`.
- `src/embed.rs` renders the `[Fonts]` section and uuencodes embedded data.
- `src/error.rs` is the shared error surface using `thiserror` and the crate-wide `Result<T>` alias.

## CLI Options (Current Implementation)

### Default Command (RunOptions)

| Flag | Default | Description |
|------|---------|-------------|
| `-i, --input` | (required) | Input ASS files (one or more) |
| `-o, --output` | `.` | Output directory |
| `-f, --fontpath` | (optional) | Font directories to scan |
| `-d, --dbpath` | `.` | Directory containing `fonts.db` |
| `--strict` | `true` | Fail on font usage errors |
| `--report` | `false` | Generate `run-report.json` after processing |
| `--force` | `false` | Overwrite existing output files |

### build Subcommand (BuildOptions)

| Flag | Default | Description |
|------|---------|-------------|
| `-f, --fontpath` | (required) | Font directories to scan (one or more) |
| `-d, --dbpath` | `.` | Output directory for `fonts.db` |

## Output Structure

Given `-o ./out` and input `sample.ass`:

- `out/sample.ass` — processed ASS with embedded fonts
- `out/run-report.json` — processing report (if `--report`)

## Conventions

- Follow the existing error-handling pattern: return `Result<T>` and use `AssfontsError` variants instead of ad hoc strings where a concrete variant already exists.
- Preserve the current import/layout style. The repository uses `rustfmt.toml` with edition 2024 and crate-level import grouping.
- Prefer extending the current pipeline helpers in `src/commands.rs` over duplicating per-input orchestration logic elsewhere.
- Keep font-name matching behavior compatible with `normalize_font_name` in `src/font.rs`.
- Add or update focused unit tests near the affected module when behavior changes.

## Dependencies

- `clap` — CLI argument parsing (derive feature)
- `regex` — pattern matching
- `serde` / `serde_json` — serialization for index and reports
- `thiserror` — error type derivation
- `walkdir` — recursive directory traversal
- `ttf-parser` — font parsing and metadata extraction
- `allsorts` — font subsetting
- `rayon` — parallel processing
- `dashmap` — concurrent hash map for font byte caching

## Pitfalls

- **README.md is outdated**: It references a `run` subcommand and options (`--subset-only`, `--embed-only`, `--luminance`, `--multi-thread`, `--rename`) that do not exist in the current implementation. It also mentions a `scripts/` directory that is not present in the repository.
- In debug builds, `src/test.rs` can intercept the first CLI argument `analyze_ass`; keep that in mind when debugging argument parsing.
- TTC/OTC handling depends on `face_index`; avoid collapsing multi-face fonts into path-only identity.
- `src/commands.rs` uses `rayon` and `DashMap` for cross-file processing and font-byte caching. Be careful not to introduce shared-state assumptions that break parallel execution.
- The `--force` flag logic is inverted: without `--force`, the code returns `FileExists` error if output already exists.

## Key Files

- `src/commands.rs` — orchestration, validation, reporting, and parallel processing
- `src/ass.rs` — ASS parsing and override-tag handling (`\fn`, `\b`, `\i`, `\r`)
- `src/font.rs` — font discovery, matching, TTC/OTC extraction
- `src/subset.rs` — allsorts subsetting wrapper
- `src/embed.rs` — uuencoding and `[Fonts]` section generation
- `src/error.rs` — project-standard error design
- `examples/sample.ass` — minimal in-repo end-to-end sample input

## Validation

- `cargo test` passes in the current workspace
- After code changes, prefer validating with `cargo test` and only add broader commands when the task specifically needs them
