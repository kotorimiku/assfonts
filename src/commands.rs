use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use color_eyre::eyre::WrapErr;
use dashmap::DashMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use ttf_parser::Face;

use crate::{
    ass::{AssAnalysis, FontRequest, analyze_ass},
    bail,
    cli::{BuildOptions, RunOptions},
    embed::{EmbeddedFont, render_ass_with_fonts},
    error::{AssfontsError, Result},
    font::{
        FontRecord, discover_fonts, extract_face_as_standalone_sfnt, match_best_font,
        normalize_font_name,
    },
    subset,
};

#[derive(Debug, Serialize, Deserialize)]
struct BuildIndex {
    total_fonts: usize,
    fonts: Vec<FontRecord>,
}

#[derive(Debug, Serialize)]
struct RunReport {
    input: PathBuf,
    output_ass: Option<PathBuf>,
    missing_fonts: Vec<String>,
    missing_sample_index: Vec<usize>,
    error_index: Vec<usize>,
    glyph_coverage: Vec<GlyphCoverageReport>,
}

#[derive(Debug, Serialize)]
struct GlyphCoverageReport {
    requested_font: String,
    requested_bold: i32,
    requested_italic: bool,
    resolved_font_file: PathBuf,
    required_count: usize,
    supported_count: usize,
    missing_sample: Vec<String>,
    original_bytes: usize,
    subset_bytes: usize,
    error: Option<String>,
}

struct FontProcessResult {
    embedded_font: EmbeddedFont,
    coverage: Option<GlyphCoverageReport>,
}

type ResolvedFontKey = (PathBuf, u32);

#[derive(Debug, Clone)]
struct PendingFontProcess {
    request: FontRequest,
    normalized_name: String,
    record: FontRecord,
    required_chars: HashSet<char>,
}

struct FontProcessContext<'a> {
    font_bytes_cache: &'a DashMap<PathBuf, Vec<u8>>,
}

pub fn run_build(options: BuildOptions) -> Result<()> {
    validate_fontpaths(&options.fontpaths)?;

    fs::create_dir_all(&options.output)?;
    let fonts = discover_fonts(&options.fontpaths);
    let index = BuildIndex {
        total_fonts: fonts.len(),
        fonts,
    };

    let index_file = options.output.join("fonts.index.json");
    fs::write(&index_file, serde_json::to_vec_pretty(&index)?)?;

    Ok(())
}

pub fn run_process(options: RunOptions) -> Result<()> {
    validate_output_dir(&options.output)?;

    let mut ass_files: Vec<(PathBuf, PathBuf)> = Vec::new();
    for input in &options.inputs {
        if input.is_dir() {
            for entry in walkdir::WalkDir::new(input) {
                let entry =
                    entry.wrap_err(format!("failed to scan directory: {}", input.display()))?;
                if entry.file_type().is_file()
                    && entry.path().extension().and_then(|s| s.to_str()) == Some("ass")
                {
                    ass_files.push((entry.path().to_path_buf(), input.clone()));
                }
            }
        } else if input.is_file() && input.extension().and_then(|s| s.to_str()) == Some("ass") {
            let base_dir = input.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            ass_files.push((input.clone(), base_dir));
        }
    }

    let fonts = load_fonts_for_run(&options)?;
    let font_bytes_cache: DashMap<PathBuf, Vec<u8>> = DashMap::new();

    let reports: Vec<RunReport> = ass_files
        .par_iter()
        .map(|(input, base_dir)| {
            process_single_input(input, base_dir, &options, &fonts, &font_bytes_cache)
                .wrap_err(format!("ass path: {}", input.display()))
        })
        .collect::<Result<Vec<_>>>()?;

    if options.report {
        let report_path = options.output.join("run-report.json");
        fs::write(report_path, serde_json::to_vec_pretty(&reports)?)?;
    }

    Ok(())
}

fn process_single_input(
    input: &std::path::Path,
    base_dir: &std::path::Path,
    options: &RunOptions,
    fonts: &[FontRecord],
    font_bytes_cache: &DashMap<PathBuf, Vec<u8>>,
) -> Result<RunReport> {
    let processed_content = load_and_preprocess_ass(input)?;

    let analysis = analyze_ass(&processed_content);
    let mut embedded_fonts = Vec::new();
    let mut glyph_coverage = Vec::new();
    let (grouped, missing_fonts) = build_pending_processes(&analysis, fonts);

    if options.strict && !missing_fonts.is_empty() {
        bail!(AssfontsError::MissingFonts(
            input.to_path_buf(),
            missing_fonts
        ));
    }

    let context = FontProcessContext { font_bytes_cache };

    for pending in grouped {
        let processed = process_font_request(
            options,
            &pending.request,
            &pending.required_chars,
            &pending.normalized_name,
            &pending.record,
            &context,
        )?;

        embedded_fonts.push(processed.embedded_font);
        if let Some(coverage) = processed.coverage {
            glyph_coverage.push(coverage);
        }
    }

    let output_ass = write_output_ass(
        options,
        input,
        base_dir,
        &processed_content,
        &embedded_fonts,
    )?;

    let mut missing_sample_index = Vec::new();
    let mut error_index = Vec::new();
    for (i, coverage) in glyph_coverage.iter().enumerate() {
        if !coverage.missing_sample.is_empty() {
            missing_sample_index.push(i);
        }
        if coverage.error.is_some() {
            error_index.push(i);
        }
    }

    Ok(RunReport {
        input: input.to_path_buf(),
        output_ass,
        missing_fonts,
        missing_sample_index,
        error_index,
        glyph_coverage,
    })
}

fn load_and_preprocess_ass(input: &std::path::Path) -> Result<String> {
    let bytes =
        fs::read(input).wrap_err_with(|| format!("failed to read file: {}", input.display()))?;
    let content = String::from_utf8(bytes)
        .wrap_err_with(|| format!("file is not valid UTF-8: {}", input.display()))?;
    Ok(content)
}

fn write_output_ass(
    options: &RunOptions,
    input: &std::path::Path,
    base_dir: &std::path::Path,
    processed_content: &str,
    embedded_fonts: &[EmbeddedFont],
) -> Result<Option<PathBuf>> {
    let path = output_ass_path(&options.output, input, base_dir);
    let rendered = render_ass_with_fonts(processed_content, embedded_fonts);

    if !options.force && fs::exists(&path)? {
        bail!(AssfontsError::FileExists(path));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, rendered.as_bytes())?;
    Ok(Some(path))
}

fn process_font_request(
    options: &RunOptions,
    request: &FontRequest,
    required_chars: &HashSet<char>,
    normalized_name: &str,
    record: &FontRecord,
    context: &FontProcessContext<'_>,
) -> Result<FontProcessResult> {
    let extension = record
        .path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("font");

    let name_base = record
        .path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or(normalized_name);

    let original_bytes = get_or_load_font_bytes(&record.path, context.font_bytes_cache)?;
    let subset_bytes = try_subset_font(&original_bytes, record.face_index, Some(required_chars));

    let embed_name = format!("{}[{}].{}", name_base, record.face_index, extension);

    let (coverage, subset_bytes) = build_coverage_report(
        options,
        request,
        record,
        required_chars,
        &original_bytes,
        subset_bytes,
    )?;

    Ok(FontProcessResult {
        embedded_font: EmbeddedFont {
            font_name: embed_name,
            bytes: subset_bytes,
        },
        coverage,
    })
}

fn build_coverage_report(
    options: &RunOptions,
    request: &FontRequest,
    record: &FontRecord,
    required_chars: &HashSet<char>,
    original_bytes: &[u8],
    subset_bytes: Result<Vec<u8>>,
) -> Result<(Option<GlyphCoverageReport>, Vec<u8>)> {
    let (subset_bytes, error) = match subset_bytes {
        Ok(bytes) => (bytes, None),
        Err(err) => {
            if options.strict && !options.allow_error_fonts {
                return Err(err).wrap_err(format!("font path: {}", record.path.display()));
            }
            (original_bytes.to_vec(), Some(err))
        }
    };

    if !options.strict && !options.report {
        return Ok((None, subset_bytes));
    }

    let coverage_face_index = record.face_index;
    let (required_count, supported_count, missing_sample) =
        measure_font_coverage(&subset_bytes, coverage_face_index, Some(required_chars));

    if (options.strict && !options.allow_missing_sample) && !missing_sample.is_empty() {
        return Err(AssfontsError::Font(format!(
            "Font '{}' is missing required characters: {:?}",
            record.display_name, missing_sample
        )))
        .wrap_err(format!("font path: {}", record.path.display()));
    }

    let coverage = match options.report {
        true => Some(GlyphCoverageReport {
            requested_font: request.font_name.clone(),
            requested_bold: request.bold,
            requested_italic: request.italic,
            resolved_font_file: record.path.clone(),
            required_count,
            supported_count,
            missing_sample,
            original_bytes: original_bytes.len(),
            subset_bytes: subset_bytes.len(),
            error: error.map(|e| e.to_string()),
        }),
        false => None,
    };

    Ok((coverage, subset_bytes))
}

fn make_resolved_key(record: &FontRecord) -> ResolvedFontKey {
    (record.path.clone(), record.face_index)
}

fn build_pending_processes(
    analysis: &AssAnalysis,
    fonts: &[FontRecord],
) -> (Vec<PendingFontProcess>, Vec<String>) {
    let mut grouped: Vec<PendingFontProcess> = Vec::new();
    let mut grouped_index: HashMap<ResolvedFontKey, usize> = HashMap::new();
    let mut missing_fonts = Vec::new();

    for (request, required_chars) in &analysis.font_requests {
        let normalized = normalize_font_name(&request.font_name);
        if let Some(record) = match_best_font(fonts, request) {
            let resolved_key = make_resolved_key(record);

            upsert_pending_process(
                &mut grouped,
                &mut grouped_index,
                resolved_key,
                request,
                &normalized,
                record,
                required_chars,
            );
        } else {
            missing_fonts.push(format_request(request));
        }
    }

    (grouped, missing_fonts)
}

fn upsert_pending_process(
    grouped: &mut Vec<PendingFontProcess>,
    grouped_index: &mut HashMap<ResolvedFontKey, usize>,
    resolved_key: ResolvedFontKey,
    request: &FontRequest,
    normalized_name: &str,
    record: &FontRecord,
    required_chars: &HashSet<char>,
) {
    if let Some(existing_index) = grouped_index.get(&resolved_key).copied() {
        grouped[existing_index]
            .required_chars
            .extend(required_chars.iter());
        return;
    }

    let index = grouped.len();
    grouped.push(PendingFontProcess {
        request: request.clone(),
        normalized_name: normalized_name.to_string(),
        record: record.clone(),
        required_chars: required_chars.clone(),
    });
    grouped_index.insert(resolved_key, index);
}

fn get_or_load_font_bytes(
    path: &std::path::Path,
    cache: &DashMap<PathBuf, Vec<u8>>,
) -> Result<Vec<u8>> {
    if let Some(cached) = cache.get(path) {
        return Ok(cached.clone());
    }

    let loaded = fs::read(path)?;
    cache.insert(path.to_path_buf(), loaded.clone());
    Ok(loaded)
}

fn measure_font_coverage(
    bytes: &[u8],
    face_index: u32,
    required_chars: Option<&HashSet<char>>,
) -> (usize, usize, Vec<String>) {
    let Some(required_chars) = required_chars else {
        return (0, 0, Vec::new());
    };

    let mut supported_count = 0usize;
    let mut missing_sample = Vec::new();

    let face = Face::parse(bytes, face_index).ok();
    for character in required_chars {
        let supported = face
            .as_ref()
            .and_then(|f| f.glyph_index(*character))
            .is_some();

        if supported {
            supported_count += 1;
        } else if missing_sample.len() < 12 {
            missing_sample.push(character.to_string());
        }
    }

    (required_chars.len(), supported_count, missing_sample)
}

fn try_subset_font(
    original_bytes: &[u8],
    face_index: u32,
    required_chars: Option<&HashSet<char>>,
) -> Result<Vec<u8>> {
    let Some(required_chars) = required_chars else {
        return Ok(Vec::new());
    };

    if required_chars.is_empty() {
        return Ok(Vec::new());
    }

    let base_font = extract_face_as_standalone_sfnt(original_bytes, face_index)?;

    subset::subset(&base_font, required_chars)
}

fn format_request(request: &FontRequest) -> String {
    format!(
        "{} (bold={}, italic={})",
        request.font_name, request.bold, request.italic
    )
}

fn output_ass_path(
    output_dir: &std::path::Path,
    input: &std::path::Path,
    base_dir: &std::path::Path,
) -> PathBuf {
    if let Ok(relative) = input.strip_prefix(base_dir) {
        output_dir.join(relative)
    } else {
        let file_name = input
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("output.ass");
        output_dir.join(file_name)
    }
}

fn validate_fontpaths(paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        if !path.exists() || !path.is_dir() {
            bail!(AssfontsError::MissingFontPath(path.clone()));
        }
    }
    Ok(())
}

fn load_fonts_for_run(options: &RunOptions) -> Result<Vec<FontRecord>> {
    let db_fonts = load_fonts_from_db(&options.dbpath)?;
    let fontpaths = options.fontpaths.as_deref().unwrap_or(&[]);

    if fontpaths.is_empty() {
        if db_fonts.is_empty() {
            bail!(AssfontsError::MissingFontSource(options.dbpath.clone()));
        }
        return Ok(db_fonts);
    }

    validate_fontpaths(fontpaths)?;
    let scanned = discover_fonts(fontpaths);
    if scanned.is_empty() && db_fonts.is_empty() {
        bail!(AssfontsError::MissingFontSource(options.dbpath.clone()));
    }

    let mut merged = scanned;
    let mut existing = std::collections::HashSet::new();
    for font in &merged {
        existing.insert((font.path.clone(), font.face_index));
    }

    for font in db_fonts {
        if !existing.contains(&(font.path.clone(), font.face_index)) && font.path.exists() {
            existing.insert((font.path.clone(), font.face_index));
            merged.push(font);
        }
    }

    Ok(merged)
}

fn load_fonts_from_db(dbpath: &std::path::Path) -> Result<Vec<FontRecord>> {
    let index_file = dbpath.join("fonts.index.json");
    if !index_file.exists() {
        return Ok(Vec::new());
    }

    let bytes = fs::read(index_file)?;
    let parsed: BuildIndex = serde_json::from_slice(&bytes)?;
    Ok(parsed
        .fonts
        .into_iter()
        .filter(|font| font.path.exists())
        .collect())
}

fn validate_output_dir(path: &std::path::Path) -> Result<()> {
    if path.exists() && !path.is_dir() {
        bail!(AssfontsError::InvalidOutputDir(path.to_path_buf()));
    }
    fs::create_dir_all(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::PathBuf};

    use super::{PendingFontProcess, ResolvedFontKey, make_resolved_key, upsert_pending_process};
    use crate::{ass::FontRequest, font::FontRecord};

    #[test]
    fn resolved_key_includes_path_and_face_index() {
        let record = FontRecord {
            display_name: "Test Font".to_string(),
            normalized_name: "testfont".to_string(),
            path: PathBuf::from("C:/fonts/test.ttf"),
            face_index: 2,
            inferred_weight: 400,
            is_italic: false,
            aliases: vec!["Test Font".to_string()],
        };

        let key = make_resolved_key(&record);
        assert_eq!(key.0, PathBuf::from("C:/fonts/test.ttf"));
        assert_eq!(key.1, 2);
    }

    #[test]
    fn resolved_key_map_can_group_duplicate_fonts() {
        let key = (PathBuf::from("C:/fonts/test.ttf"), 0_u32);
        let mut grouped_index = std::collections::HashMap::new();
        grouped_index.insert(key.clone(), 0_usize);

        assert_eq!(grouped_index.get(&key), Some(&0_usize));
    }

    #[test]
    fn upsert_pending_process_merges_required_chars_for_same_key() {
        let mut grouped: Vec<PendingFontProcess> = Vec::new();
        let mut grouped_index: std::collections::HashMap<ResolvedFontKey, usize> =
            std::collections::HashMap::new();

        let record = FontRecord {
            display_name: "Test Font".to_string(),
            normalized_name: "testfont".to_string(),
            path: PathBuf::from("C:/fonts/test.ttf"),
            face_index: 0,
            inferred_weight: 400,
            is_italic: false,
            aliases: vec!["Test Font".to_string()],
        };
        let key = make_resolved_key(&record);

        let request = FontRequest {
            font_name: "Test Font".to_string(),
            bold: 400,
            italic: false,
        };

        let mut chars_a = HashSet::new();
        chars_a.insert('A');
        let mut chars_b = HashSet::new();
        chars_b.insert('B');

        upsert_pending_process(
            &mut grouped,
            &mut grouped_index,
            key.clone(),
            &request,
            "testfont",
            &record,
            &chars_a,
        );
        upsert_pending_process(
            &mut grouped,
            &mut grouped_index,
            key,
            &request,
            "testfont",
            &record,
            &chars_b,
        );

        assert_eq!(grouped.len(), 1);
        assert!(grouped[0].required_chars.contains(&'A'));
        assert!(grouped[0].required_chars.contains(&'B'));
    }
}
