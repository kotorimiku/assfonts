# assfonts

English | [简体中文](README_zh_CN.md)

A Rust desktop and CLI tool for ASS subtitle font indexing, matching, subsetting, and embedding.

## Features

- **Font Indexing**: Scan font directories and build a JSON index for fast lookups
- **Smart Font Matching**: Match ASS font requests to available fonts with bold/italic support
- **Font Subsetting**: Reduce font file size by extracting only required glyphs
- **Font Embedding**: Embed subsetted fonts directly into ASS files as `[Fonts]` section
- **Multi-file Processing**: Process multiple ASS files in parallel
- **TTC/OTC Support**: Extract individual faces from TrueType/OpenType Collection fonts
- **Desktop GUI**: Run subtitle processing and index building from a Dioxus desktop interface

## Installation

### From Source

```bash
git clone https://github.com/kotorimiku/assfonts.git
cd assfonts
cargo build --release
```

## Usage

### Desktop GUI

Launch the desktop application:

```bash
cargo run --bin assfonts_gui
```

The GUI currently includes:
- ASS processing workspace with path pickers and runtime options
- Font index building workspace
- Progress feedback and cancel support for ASS processing
- Result summary panel after each run

If you want to use the CLI with `cargo run`, specify the CLI binary explicitly because the package default target launches the GUI:

```bash
cargo run --bin assfonts -- -h
```

### Build Font Index

Build a font index file for fast font lookups:

```bash
assfonts build -f /path/to/fonts -o /path/to/db
```

This creates `fonts.index.json` in the specified directory.

Options:
- `-f, --fontpath`: Font directories to scan (required, supports multiple paths)
- `-o, --output`: Output directory for `fonts.index.json` (default: current directory)

### Process ASS Files

Embed fonts into ASS files:

```bash
assfonts -i input.ass -o output_dir -f /path/to/fonts
```

Process multiple files or directories:

```bash
assfonts -i /path/to/ass/files -o output_dir -f /path/to/fonts -d /path/to/db
```

Options:
- `-i, --input`: Input ASS files or directories (required, supports multiple paths)
- `-o, --output`: Output directory (default: current directory)
- `-f, --fontpath`: Font directories to scan (optional)
- `-d, --dbpath`: Directory containing `fonts.index.json` (default: current directory)
- `-s, --strict`: Fail on font usage errors (default: true)
- `--allow-missing-sample`: Allow missing character samples
- `--allow-missing-fonts`: Allow missing fonts
- `--report`: Generate `run-report.json` after processing
- `--force`: Overwrite existing output files

## Examples

### Basic Usage

```bash
# Process a single ASS file
assfonts -i video.ass -o ./out -f C:/Windows/Fonts

# Process all ASS files in a directory
assfonts -i ./subtitles -o ./output -f /usr/share/fonts

# Use pre-built font index
assfonts build -f /usr/share/fonts -o ~/.assfonts
assfonts -i video.ass -o ./out -d ~/.assfonts
```

### Build Specific Binaries

```bash
# CLI binary
cargo build --release --bin assfonts

# GUI binary
cargo build --release --bin assfonts_gui
```

### With Report Generation

```bash
assfonts -i video.ass -o ./out -f C:/Windows/Fonts --report
```

The report (`run-report.json`) includes:
- Missing fonts
- Glyph coverage statistics
- Font file size reduction

### Relaxed Mode

Allow missing fonts and continue processing:

```bash
assfonts -i video.ass -o ./out -f C:/Windows/Fonts --allow-missing-fonts --allow-missing-sample
```

## Related Projects

- [assfonts](https://github.com/wyzdwdz/assfonts) - Original C++ implementation

## License

MIT License - see [LICENSE](LICENSE) for details.
