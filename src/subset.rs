use std::collections::HashSet;

use allsorts::{
    binary::read::ReadScope,
    font::Font,
    font_data::FontData,
    subset::{CmapTarget, SubsetProfile, subset as allsorts_subset},
    tables::cmap::CmapSubtable,
};

use crate::{
    cff_fix::{fix_cff_font, is_cff_charstring_error},
    error::Result,
};

pub fn subset_with_allsorts(
    original_bytes: &[u8],
    required_chars: &HashSet<char>,
) -> Result<Vec<u8>> {
    let font_data = ReadScope::new(original_bytes).read::<FontData<'_>>()?;
    let provider = font_data.table_provider(0)?;
    let font = Font::new(provider)?;
    let cmap_subtable = ReadScope::new(font.cmap_subtable_data()).read::<CmapSubtable<'_>>()?;

    let mut glyph_ids = vec![0];
    for ch in required_chars {
        if let Ok(Some(glyph_id)) = cmap_subtable.map_glyph(*ch as u32) {
            glyph_ids.push(glyph_id);
        }
    }

    glyph_ids.sort_unstable();
    glyph_ids.dedup();

    Ok(allsorts_subset(
        &font.font_table_provider,
        &glyph_ids,
        &SubsetProfile::Minimal,
        CmapTarget::Unicode,
    )?)
}
pub fn subset(original_bytes: &[u8], required_chars: &HashSet<char>) -> Result<Vec<u8>> {
    // First, try subsetting directly
    match subset_with_allsorts(original_bytes, required_chars) {
        Ok(result) => Ok(result),
        Err(e) => {
            let err_str = e.to_string();
            if is_cff_charstring_error(&err_str) {
                // Try to fix CFF font
                let mut fixed_bytes = original_bytes.to_vec();
                if fix_cff_font(&mut fixed_bytes).is_some() {
                    // Try subsetting the fixed font
                    if let Ok(result) = subset_with_allsorts(&fixed_bytes, required_chars) {
                        return Ok(result);
                    }
                }
            }
            Err(e)
        }
    }
}
