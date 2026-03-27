use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use ttf_parser::{Face, fonts_in_collection, name_id};
use walkdir::WalkDir;

use crate::{
    ass::FontRequest,
    bail,
    error::{AssfontsError, Result},
};

fn align4(value: usize) -> usize {
    (value + 3) & !3
}

fn read_u16_be(bytes: &[u8], at: usize) -> Option<u16> {
    let raw = bytes.get(at..at + 2)?;
    Some(u16::from_be_bytes([raw[0], raw[1]]))
}

fn read_u32_be(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at + 4)?;
    Some(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontRecord {
    pub display_name: String,
    pub normalized_name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub face_index: u32,
    pub inferred_weight: i32,
    pub is_italic: bool,
    #[serde(default)]
    pub aliases: Vec<String>,
}

pub fn normalize_font_name(value: &str) -> String {
    let filter_chars = ['-', '_', '@', ',', ':'];
    value
        .chars()
        .filter(|c| !c.is_whitespace() && !filter_chars.contains(c))
        .flat_map(char::to_lowercase)
        .collect::<String>()
}

fn supported_font_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|x| x.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc" | "woff" | "woff2"
            )
        })
        .unwrap_or(false)
}

pub fn discover_fonts(fontpaths: &[PathBuf]) -> Vec<FontRecord> {
    let mut records = Vec::new();

    for root in fontpaths {
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if !path.is_file() || !supported_font_extension(path) {
                continue;
            }

            records.extend(discover_from_file(path));
        }
    }

    records
}

fn discover_from_file(path: &Path) -> Vec<FontRecord> {
    let Some(stem) = path.file_stem().and_then(|x| x.to_str()) else {
        return Vec::new();
    };

    let fallback_name = stem.trim().to_string();
    if fallback_name.is_empty() {
        return Vec::new();
    }

    let bytes = match std::fs::read(path) {
        Ok(value) => value,
        Err(_) => return vec![fallback_record(path, &fallback_name)],
    };

    let face_count = fonts_in_collection(&bytes).unwrap_or(1);
    let mut records = Vec::new();

    for face_index in 0..face_count {
        match Face::parse(&bytes, face_index) {
            Ok(face) => {
                let aliases = collect_aliases(&face, &fallback_name);
                let display_name = aliases
                    .first()
                    .cloned()
                    .unwrap_or_else(|| fallback_name.clone());

                records.push(FontRecord {
                    normalized_name: normalize_font_name(&display_name),
                    display_name,
                    path: path.to_path_buf(),
                    face_index,
                    inferred_weight: face.weight().to_number() as i32,
                    is_italic: face.is_italic(),
                    aliases,
                });
            }
            Err(_) => {
                if face_index == 0 {
                    records.push(fallback_record(path, &fallback_name));
                }
            }
        }
    }

    if records.is_empty() {
        records.push(fallback_record(path, &fallback_name));
    }

    records
}

fn collect_aliases(face: &Face<'_>, fallback_name: &str) -> Vec<String> {
    let mut set = BTreeSet::new();

    for name in face.names() {
        if !matches!(
            name.name_id,
            name_id::TYPOGRAPHIC_FAMILY
                | name_id::FAMILY
                | name_id::FULL_NAME
                | name_id::POST_SCRIPT_NAME
        ) {
            continue;
        }

        if let Some(text) = name.to_string() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                set.insert(trimmed.to_string());
            }
        }
    }

    if set.is_empty() {
        set.insert(fallback_name.to_string());
    }

    set.into_iter().collect()
}

fn fallback_record(path: &Path, fallback_name: &str) -> FontRecord {
    FontRecord {
        normalized_name: normalize_font_name(fallback_name),
        display_name: fallback_name.to_string(),
        path: path.to_path_buf(),
        face_index: 0,
        inferred_weight: infer_weight(fallback_name),
        is_italic: infer_italic(fallback_name),
        aliases: vec![fallback_name.to_string()],
    }
}

pub fn match_best_font<'a>(
    fonts: &'a [FontRecord],
    request: &FontRequest,
) -> Option<&'a FontRecord> {
    let target = normalize_font_name(&request.font_name);
    let mut candidates: Vec<&FontRecord> = fonts
        .iter()
        .filter(|font| {
            font.normalized_name == target
                || font
                    .aliases
                    .iter()
                    .any(|alias| normalize_font_name(alias) == target)
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let req_weight = if request.bold >= 600 {
        request.bold
    } else {
        400
    };

    candidates.sort_by_key(|font| {
        let weight_gap = (font.inferred_weight - req_weight).abs();
        let italic_penalty = if font.is_italic == request.italic {
            0
        } else {
            300
        };
        weight_gap + italic_penalty
    });

    candidates.into_iter().next()
}

fn infer_weight(name: &str) -> i32 {
    let lower = name.to_ascii_lowercase();
    if lower.contains("black") || lower.contains("heavy") {
        900
    } else if lower.contains("extrabold") || lower.contains("ultrabold") {
        800
    } else if lower.contains("semibold") || lower.contains("demibold") {
        600
    } else if lower.contains("bold") {
        700
    } else if lower.contains("medium") {
        500
    } else if lower.contains("light") {
        300
    } else if lower.contains("thin") {
        200
    } else {
        400
    }
}

fn infer_italic(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("italic") || lower.contains("oblique")
}

pub fn extract_face_as_standalone_sfnt(font_bytes: &[u8], face_index: u32) -> Result<Vec<u8>> {
    if face_index == 0 && !font_bytes.starts_with(b"ttcf") {
        return Ok(font_bytes.to_vec());
    }

    let raw_face = ttf_parser::RawFace::parse(font_bytes, face_index)
        .map_err(|e| AssfontsError::Font(format!("failed to parse face: {e:?}")))?;

    let face_offset = resolve_face_offset(font_bytes, face_index)?;
    let scaler_type = font_bytes
        .get(face_offset..face_offset + 4)
        .ok_or_else(|| AssfontsError::Font("face scaler type out of bounds".to_string()))?;

    let mut records: Vec<ttf_parser::TableRecord> = raw_face.table_records.into_iter().collect();
    if records.is_empty() {
        bail!(AssfontsError::Font("no table records in face".to_string()));
    }
    records.sort_by_key(|record| record.tag);

    let num_tables = records.len();
    if num_tables > u16::MAX as usize {
        bail!(AssfontsError::Font("too many tables".to_string()));
    }

    let max_pow2 = 1usize << ((num_tables as u32).ilog2());
    let search_range = (max_pow2 * 16) as u16;
    let entry_selector = (max_pow2 as u32).ilog2() as u16;
    let range_shift = (num_tables * 16) as u16 - search_range;

    let header_size = 12usize;
    let directory_size = num_tables * 16usize;
    let mut table_offset = header_size + directory_size;

    let mut payload = Vec::new();
    let mut directory = Vec::with_capacity(directory_size);

    for record in records {
        let start = record.offset as usize;
        let length = record.length as usize;
        let end = start
            .checked_add(length)
            .ok_or_else(|| AssfontsError::Font("table range overflow".to_string()))?;
        let table_data = font_bytes.get(start..end).ok_or_else(|| {
            AssfontsError::Font(format!(
                "table '{}' out of bounds",
                String::from_utf8_lossy(&record.tag.to_bytes())
            ))
        })?;

        let padded_len = align4(length);

        directory.extend_from_slice(&record.tag.to_bytes());
        directory.extend_from_slice(&record.check_sum.to_be_bytes());
        directory.extend_from_slice(&(table_offset as u32).to_be_bytes());
        directory.extend_from_slice(&(length as u32).to_be_bytes());

        payload.extend_from_slice(table_data);
        if padded_len > length {
            payload.resize(payload.len() + (padded_len - length), 0);
        }

        table_offset += padded_len;
    }

    let mut out = Vec::with_capacity(header_size + directory.len() + payload.len());
    out.extend_from_slice(scaler_type);
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    out.extend_from_slice(&directory);
    out.extend_from_slice(&payload);

    apply_check_sum_adjustment(&mut out)?;

    Ok(out)
}

fn apply_check_sum_adjustment(sfnt: &mut [u8]) -> Result<()> {
    if sfnt.len() < 12 {
        bail!(AssfontsError::Font("sfnt too small".to_string()));
    }

    let num_tables = read_u16_be(sfnt, 4)
        .ok_or_else(|| AssfontsError::Font("invalid sfnt header".to_string()))?;
    let dir_size = 12usize + num_tables as usize * 16usize;
    if sfnt.len() < dir_size {
        bail!(AssfontsError::Font(
            "invalid sfnt table directory".to_string()
        ));
    }

    let mut head_offset = None;
    for index in 0..num_tables as usize {
        let rec = 12usize + index * 16usize;
        let tag = sfnt
            .get(rec..rec + 4)
            .ok_or_else(|| AssfontsError::Font("table record out of bounds".to_string()))?;
        if tag == b"head" {
            let offset = read_u32_be(sfnt, rec + 8)
                .ok_or_else(|| AssfontsError::Font("head offset missing".to_string()))?
                as usize;
            let length = read_u32_be(sfnt, rec + 12)
                .ok_or_else(|| AssfontsError::Font("head length missing".to_string()))?
                as usize;

            if length < 12
                || offset
                    .checked_add(length)
                    .is_none_or(|end| end > sfnt.len())
            {
                bail!(AssfontsError::Font("invalid head table bounds".to_string()));
            }

            head_offset = Some(offset);
            break;
        }
    }

    let head_offset =
        head_offset.ok_or_else(|| AssfontsError::Font("head table not found".to_string()))?;
    let adjust_at = head_offset + 8;
    sfnt[adjust_at..adjust_at + 4].copy_from_slice(&0u32.to_be_bytes());

    let mut sum: u64 = 0;
    for chunk in sfnt.chunks(4) {
        let word = if chunk.len() == 4 {
            u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        } else {
            let mut temp = [0u8; 4];
            temp[..chunk.len()].copy_from_slice(chunk);
            u32::from_be_bytes(temp)
        };
        sum = sum.wrapping_add(word as u64);
    }

    let checksum = (sum & 0xFFFF_FFFF) as u32;
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum);
    sfnt[adjust_at..adjust_at + 4].copy_from_slice(&adjustment.to_be_bytes());
    Ok(())
}

fn resolve_face_offset(font_bytes: &[u8], face_index: u32) -> Result<usize> {
    if !font_bytes.starts_with(b"ttcf") {
        if face_index == 0 {
            return Ok(0);
        }
        bail!(AssfontsError::Font(format!(
            "face index out of bounds for single font: {face_index}"
        )));
    }

    if font_bytes.len() < 12 {
        bail!(AssfontsError::Font("invalid TTC header".to_string()));
    }

    let num_faces =
        u32::from_be_bytes([font_bytes[8], font_bytes[9], font_bytes[10], font_bytes[11]]);

    if face_index >= num_faces {
        bail!(AssfontsError::Font(format!(
            "face index out of bounds: {face_index} >= {num_faces}"
        )));
    }

    let table_offset = 12usize + face_index as usize * 4;
    let face_offset = font_bytes
        .get(table_offset..table_offset + 4)
        .ok_or_else(|| AssfontsError::Font("invalid TTC face offset table".to_string()))?;

    let offset = u32::from_be_bytes([
        face_offset[0],
        face_offset[1],
        face_offset[2],
        face_offset[3],
    ]) as usize;

    if offset + 4 > font_bytes.len() {
        bail!(AssfontsError::Font("invalid TTC face offset".to_string()));
    }

    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::{extract_face_as_standalone_sfnt, resolve_face_offset};
    use crate::error::AssfontsError;

    #[test]
    fn resolve_ttc_face_offset_works() {
        let mut data = Vec::new();
        data.extend_from_slice(b"ttcf");
        data.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        data.extend_from_slice(&2u32.to_be_bytes());
        data.extend_from_slice(&32u32.to_be_bytes());
        data.extend_from_slice(&96u32.to_be_bytes());
        data.resize(100, 0);

        let got = resolve_face_offset(&data, 1).expect("face offset should parse");
        assert_eq!(got, 96);
    }

    #[test]
    fn resolve_ttc_face_offset_out_of_bounds() {
        let mut data = Vec::new();
        data.extend_from_slice(b"ttcf");
        data.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&32u32.to_be_bytes());
        data.resize(40, 0);

        let err = resolve_face_offset(&data, 1).expect_err("should reject out of bounds");
        let inner = err.downcast_ref::<AssfontsError>();
        assert!(matches!(inner, Some(AssfontsError::Font(_))));
    }

    #[test]
    fn extract_single_font_face_zero_passthrough() {
        let raw = b"not-a-ttcf".to_vec();
        let out = extract_face_as_standalone_sfnt(&raw, 0).expect("face0 passthrough expected");
        assert_eq!(out, raw);
    }
}
