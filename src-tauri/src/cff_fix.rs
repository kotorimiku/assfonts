//! CFF font repair module
//!
//! This module handles fixing CFF fonts that have invalid charstrings
//! (extra data after endchar operator) which causes subsetting to fail.

/// CFF endchar operator byte value
const ENDCHAR: u8 = 14;

#[derive(Debug)]
struct IndexInfo {
    count: usize,
    off_size: usize,
    offsets_start: usize,
    data_start: usize,
    data_length: usize,
    index_size: usize,
}

/// Find the offset and length of a table in the font file by tag
fn find_table(data: &[u8], tag: u32) -> Option<(usize, usize)> {
    if data.len() < 12 {
        return None;
    }

    let sfnt_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if sfnt_version != 0x00010000 && sfnt_version != 0x4F54544F {
        return None;
    }

    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;

    for i in 0..num_tables {
        let record_offset = 12 + i * 16;
        if record_offset + 16 > data.len() {
            break;
        }

        let table_tag = u32::from_be_bytes([
            data[record_offset],
            data[record_offset + 1],
            data[record_offset + 2],
            data[record_offset + 3],
        ]);

        if table_tag == tag {
            let offset = u32::from_be_bytes([
                data[record_offset + 8],
                data[record_offset + 9],
                data[record_offset + 10],
                data[record_offset + 11],
            ]) as usize;
            let length = u32::from_be_bytes([
                data[record_offset + 12],
                data[record_offset + 13],
                data[record_offset + 14],
                data[record_offset + 15],
            ]) as usize;
            return Some((offset, length));
        }
    }

    None
}

/// Read a CFF INDEX offset value
fn read_offset(data: &[u8], pos: usize, off_size: usize) -> usize {
    if pos + off_size > data.len() {
        return 0;
    }
    match off_size {
        1 => data[pos] as usize,
        2 => u16::from_be_bytes([data[pos], data[pos + 1]]) as usize,
        3 => {
            ((data[pos] as usize) << 16)
                | ((data[pos + 1] as usize) << 8)
                | (data[pos + 2] as usize)
        }
        4 => u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize,
        _ => 0,
    }
}

/// Parse a CFF charstring and find the position after endchar
fn find_charstring_end(data: &[u8]) -> Option<usize> {
    let mut pos = 0;
    let mut stem_count = 0u32;
    let mut operand_count = 0usize;

    while pos < data.len() {
        let b = data[pos];

        match b {
            ENDCHAR => return Some(pos + 1),
            19 | 20 => {
                // hintmask/countermask operators
                stem_count += (operand_count / 2) as u32;
                operand_count = 0;
                let hint_bytes = stem_count.div_ceil(8) as usize;
                if pos + 1 + hint_bytes > data.len() {
                    return find_endchar_fallback(data, pos);
                }
                pos += 1 + hint_bytes;
            }
            1 | 3 | 18 | 23 => {
                // stem operators
                stem_count += (operand_count / 2) as u32;
                operand_count = 0;
                pos += 1;
            }
            12 => {
                // Two-byte operator
                if pos + 1 < data.len() {
                    operand_count = 0;
                    pos += 2;
                } else {
                    break;
                }
            }
            0 | 2 | 4..=11 | 13 | 15..=17 | 21 => {
                // Single-byte operators
                operand_count = 0;
                pos += 1;
            }
            32..=246 => {
                // Single-byte number
                operand_count += 1;
                pos += 1;
            }
            247..=250 => {
                // Two-byte positive number
                operand_count += 1;
                pos += 2;
            }
            251..=254 => {
                // Two-byte negative number
                operand_count += 1;
                pos += 2;
            }
            28 => {
                // Three-byte number
                operand_count += 1;
                pos += 3;
            }
            29 => {
                // Five-byte number
                operand_count += 1;
                pos += 5;
            }
            22 | 24..=31 => {
                // Reserved operators (including 30 which was BCD in Type 1)
                operand_count = 0;
                pos += 1;
            }
            255 => {
                // Fixed number (4 bytes) in Type 2
                operand_count += 1;
                pos += 5;
            }
        }
    }

    // Fallback: search for endchar at the end
    find_endchar_fallback(data, 0)
}

/// Fallback: search for endchar byte (14) starting from a position
/// Returns the position after endchar if found at the end of data
fn find_endchar_fallback(data: &[u8], start: usize) -> Option<usize> {
    if data.is_empty() {
        return None;
    }

    // Check if last byte is endchar
    if *data.last()? == ENDCHAR {
        return Some(data.len());
    }

    // Check if second-to-last byte is endchar (in case of malformed BCD)
    if data.len() >= 2 && data[data.len() - 2] == ENDCHAR {
        return Some(data.len() - 1);
    }

    // Search from start position for endchar near the end
    for i in start..data.len() {
        if data[i] == ENDCHAR && i >= data.len() - 10 {
            return Some(i + 1);
        }
    }

    None
}

/// Skip a CFF INDEX, returning the position after it
fn skip_index(data: &[u8], pos: usize) -> Option<usize> {
    let info = find_index_data_info(data, pos)?;

    Some(info.index_size + pos)
}

fn find_index_data_info(data: &[u8], index_start: usize) -> Option<IndexInfo> {
    if index_start + 2 > data.len() {
        return None;
    }

    let count = u16::from_be_bytes([data[index_start], data[index_start + 1]]) as usize;
    let pos = index_start + 2;

    if count == 0 {
        return Some(IndexInfo {
            count: 0,
            off_size: 0,
            offsets_start: 0,
            data_start: 0,
            data_length: 0,
            index_size: 2,
        });
    }

    if pos >= data.len() {
        return None;
    }

    let off_size = data[pos] as usize;
    let offsets_start = pos + 1;

    if off_size == 0 || off_size > 4 {
        return None;
    }

    // offsets array should 1-based, so first offset should be 1
    if read_offset(data, offsets_start, off_size) != 1 {
        return None;
    }

    // Read last offset to get total size
    let last_off_pos = offsets_start + count * off_size;
    if last_off_pos + off_size > data.len() {
        return None;
    }

    let last_offset = read_offset(data, last_off_pos, off_size);

    // CFF INDEX offsets are 1-based, so actual data length is last_offset - 1
    let data_length = last_offset.saturating_sub(1);

    let data_start = offsets_start + (count + 1) * off_size;

    if data_start + data_length > data.len() {
        return None;
    }

    Some(IndexInfo {
        count,
        off_size,
        offsets_start,
        data_start,
        data_length,
        index_size: 2 + 1 + (count + 1) * off_size + data_length,
    })
}

/// Parse Top DICT to find CharStrings offset (operator 17)
fn find_charstrings_offset(data: &[u8], dict_start: usize, dict_end: usize) -> Option<usize> {
    if dict_start >= dict_end || dict_end > data.len() {
        return None;
    }

    let mut pos = dict_start;
    let mut operands: Vec<i32> = Vec::new();

    while pos < dict_end {
        let b0 = data[pos];

        match b0 {
            12 => {
                if pos + 1 >= dict_end {
                    return None;
                }
                operands.clear();
                pos += 2;
            }

            0..=21 => {
                if b0 == 17 {
                    if operands.len() != 1 {
                        return None;
                    }
                    return Some(operands[0] as usize);
                }

                operands.clear();
                pos += 1;
            }

            32..=246 => {
                operands.push((b0 as i32) - 139);
                pos += 1;
            }

            247..=250 => {
                if pos + 1 >= dict_end {
                    return None;
                }

                let val = (b0 as i32 - 247) * 256 + data[pos + 1] as i32 + 108;

                operands.push(val);
                pos += 2;
            }

            251..=254 => {
                if pos + 1 >= dict_end {
                    return None;
                }

                let val = -(b0 as i32 - 251) * 256 - data[pos + 1] as i32 - 108;

                operands.push(val);
                pos += 2;
            }

            28 => {
                if pos + 2 >= dict_end {
                    return None;
                }

                let val = i16::from_be_bytes([data[pos + 1], data[pos + 2]]) as i32;

                operands.push(val);
                pos += 3;
            }

            29 => {
                if pos + 4 >= dict_end {
                    return None;
                }

                let val = i32::from_be_bytes([
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                ]);

                operands.push(val);
                pos += 5;
            }

            30 => {
                pos += 1;

                while pos < dict_end {
                    let byte = data[pos];
                    pos += 1;

                    let high = byte >> 4;
                    let low = byte & 0x0F;

                    if high == 0xF || low == 0xF {
                        break;
                    }
                }
            }

            255 => return None,

            _ => return None,
        }
    }

    None
}

/// Rebuild CFF CharStrings INDEX at a specific offset
fn rebuild_charstrings_index_at(
    data: &mut Vec<u8>,
    cff_offset: usize,
    charstrings_offset: usize,
) -> bool {
    let abs_offset = cff_offset + charstrings_offset;

    if abs_offset >= data.len() {
        return false;
    }

    // First pass: read and analyze the index
    let info = match find_index_data_info(data, abs_offset) {
        Some(info) => info,
        None => return false,
    };

    let count = info.count;
    let offsets = (0..=count)
        .map(|i| read_offset(data, info.offsets_start + i * info.off_size, info.off_size))
        .collect::<Vec<usize>>();

    if offsets.len() != count + 1 {
        return false;
    }

    // Convert absolute positions to relative positions (relative to abs_offset)
    let data_start_rel = info.data_start.saturating_sub(abs_offset);
    let old_index_size = info.index_size;

    // Calculate new charstring lengths
    let mut new_lengths: Vec<usize> = Vec::with_capacity(count);
    let mut total_new_size = 0usize;
    let mut truncated_count = 0usize;

    {
        let index_data = &data[abs_offset..];

        for i in 0..count {
            let start = data_start_rel + offsets[i].saturating_sub(1);
            let end = data_start_rel + offsets[i + 1].saturating_sub(1);

            if start >= index_data.len() || end > index_data.len() || start >= end {
                new_lengths.push(0);
                continue;
            }

            let charstring = &index_data[start..end];
            let new_len = find_charstring_end(charstring).unwrap_or(charstring.len());
            // let p = charstring
            //     .iter()
            //     .position(|b| b == &ENDCHAR)
            //     .unwrap_or(charstring.len());

            if new_len < charstring.len() {
                truncated_count += 1;
            }
            new_lengths.push(new_len);
            total_new_size += new_len;
        }
    }

    if truncated_count == 0 {
        return false;
    }

    // Calculate new offset size
    let new_off_size = if total_new_size + 1 < 256 {
        1
    } else if total_new_size + 1 < 65536 {
        2
    } else if total_new_size + 1 < 16777216 {
        3
    } else {
        4
    };

    // Calculate sizes
    let new_header_size = 3 + (count + 1) * new_off_size;
    let new_index_size = new_header_size + total_new_size;
    let size_diff = old_index_size.saturating_sub(new_index_size);

    // Build new index in a separate buffer
    let mut new_index = vec![0u8; new_index_size];

    // Write count
    new_index[0] = (count >> 8) as u8;
    new_index[1] = count as u8;
    new_index[2] = new_off_size as u8;

    // Write offsets
    let mut current_offset = 1usize;
    for i in 0..=count {
        let off_pos = 3 + i * new_off_size;
        match new_off_size {
            1 => new_index[off_pos] = current_offset as u8,
            2 => {
                new_index[off_pos] = (current_offset >> 8) as u8;
                new_index[off_pos + 1] = current_offset as u8;
            }
            3 => {
                new_index[off_pos] = (current_offset >> 16) as u8;
                new_index[off_pos + 1] = (current_offset >> 8) as u8;
                new_index[off_pos + 2] = current_offset as u8;
            }
            4 => {
                new_index[off_pos] = (current_offset >> 24) as u8;
                new_index[off_pos + 1] = (current_offset >> 16) as u8;
                new_index[off_pos + 2] = (current_offset >> 8) as u8;
                new_index[off_pos + 3] = current_offset as u8;
            }
            _ => {}
        }
        if let Some(&len) = new_lengths.get(i) {
            current_offset += len;
        }
    }

    // Copy truncated charstrings
    {
        let index_data = &data[abs_offset..];
        let mut dest_pos = new_header_size;

        for i in 0..count {
            let start = data_start_rel + offsets[i].saturating_sub(1);
            if start + new_lengths[i] <= index_data.len() && new_lengths[i] > 0 {
                new_index[dest_pos..dest_pos + new_lengths[i]]
                    .copy_from_slice(&index_data[start..start + new_lengths[i]]);
            }
            dest_pos += new_lengths[i];
        }
    }

    // Replace the old CharStrings INDEX with the new one
    // Add padding at the end to maintain CFF table size
    let padding: Vec<u8> = vec![0; size_diff];

    // Replace in place: remove old index, insert new index + padding
    data.splice(
        abs_offset..abs_offset + old_index_size,
        new_index.into_iter().chain(padding),
    );

    true
}

/// Fix CFF font by truncating charstrings after endchar
pub fn fix_cff_font(data: &mut Vec<u8>) -> Option<()> {
    // Find CFF table
    let cff_table = find_table(data, 0x43464620)?;

    let (cff_offset, _) = cff_table;

    if cff_offset >= data.len() {
        return None;
    }

    // First pass: collect info from CFF data (immutable borrow)
    let charstrings_offset = {
        let cff_data = &data[cff_offset..];
        if cff_data.len() < 4 {
            return None;
        }

        // CFF header: major(1), minor(1), hdrSize(1), offSize(1)
        let header_size = cff_data[2] as usize;
        let header_size = header_size.max(4);

        // Skip Name INDEX
        let pos = skip_index(cff_data, header_size)?;

        // Get Top DICT INDEX bounds
        let top_dict_index_start = pos;

        // Find Top DICT data offset and length
        let info = find_index_data_info(cff_data, top_dict_index_start)?;
        let (dict_start, data_length) = (info.data_start, info.data_length);
        let dict_end = dict_start + data_length;

        // Find CharStrings offset
        find_charstrings_offset(cff_data, dict_start, dict_end)?
    };

    if rebuild_charstrings_index_at(data, cff_offset, charstrings_offset) {
        return Some(());
    }

    None
}

/// Check if an error is related to CFF charstring issues
pub fn is_cff_charstring_error(error: &str) -> bool {
    error.contains("CFF") || error.contains("endchar") || error.contains("unused data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_index() {
        // Valid INDEX with 2 items, offSize=1, data "abc", "de"
        let data = [
            0x00, 0x02, // count = 2
            0x01, // offSize = 1
            0x01, 0x04, 0x06, // offsets: 1, 4, 6
            b'a', b'b', b'c', // item 1
            b'd', b'e', // item 2
        ];
        assert_eq!(skip_index(&data, 0), Some(11));
    }

    #[test]
    fn test_skip_index2() {
        // Valid INDEX with 2 items, offSize=1, data "abc", "de"
        let data = [
            0x00, 0x02, // count = 2
            0x02, // offSize = 2
            0x00, 0x01, 0x00, 0x04, 0x00, 0x06, // offsets: 1, 4, 6
            b'a', b'b', b'c', // item 1
            b'd', b'e', // item 2
        ];
        assert_eq!(skip_index(&data, 0), Some(14));
    }

    #[test]
    fn test_skip_index_empty() {
        // Empty INDEX
        let data = [0x00, 0x00]; // count = 0
        assert_eq!(skip_index(&data, 0), Some(2));
    }

    #[test]
    fn test_find_charstring_end_simple() {
        // Simple charstring: just endchar
        let data = [14]; // endchar
        assert_eq!(find_charstring_end(&data), Some(1));
    }

    #[test]
    fn test_find_charstring_end_with_number() {
        // Number (139) + endchar
        let data = [139, 14]; // 0, endchar
        assert_eq!(find_charstring_end(&data), Some(2));
    }

    #[test]
    fn test_find_charstring_end_no_endchar() {
        // No endchar
        let data = [139, 140]; // 0, 1
        assert_eq!(find_charstring_end(&data), None);
    }

    #[test]
    fn test_is_cff_charstring_error() {
        assert!(is_cff_charstring_error("CFF error"));
        assert!(is_cff_charstring_error(
            "unused data left after 'endchar' operator"
        ));
        assert!(!is_cff_charstring_error("some other error"));
    }

    #[test]
    fn test_find_charstrings_offset_invalid_bounds() {
        let data = [0x00, 0x01, 0x02];
        // dict_start >= dict_end
        assert_eq!(find_charstrings_offset(&data, 2, 1), None);
        // dict_end > data.len()
        assert_eq!(find_charstrings_offset(&data, 0, 100), None);
    }

    #[test]
    fn test_find_charstrings_offset_single_byte_operand() {
        // Single-byte number (32-246): value = b0 - 139
        // 139 -> value = 0, operator 17 (CharStrings)
        let data = [139, 17];
        assert_eq!(find_charstrings_offset(&data, 0, 2), Some(0));
    }

    #[test]
    fn test_find_charstrings_offset_single_byte_positive() {
        // 246 -> value = 246 - 139 = 107
        let data = [246, 17];
        assert_eq!(find_charstrings_offset(&data, 0, 2), Some(107));
    }

    #[test]
    fn test_find_charstrings_offset_single_byte_negative() {
        // 32 -> value = 32 - 139 = -107
        let data = [32, 17];
        assert_eq!(
            find_charstrings_offset(&data, 0, 2),
            Some((-107i32) as usize)
        );
    }

    #[test]
    fn test_find_charstrings_offset_two_byte_positive() {
        // Two-byte positive (247-250): value = (b0-247)*256 + b1 + 108
        // 247, 0 -> value = 0*256 + 0 + 108 = 108
        let data = [247, 0, 17];
        assert_eq!(find_charstrings_offset(&data, 0, 3), Some(108));
    }

    #[test]
    fn test_find_charstrings_offset_two_byte_negative() {
        // Two-byte negative (251-254): value = -(b0-251)*256 - b1 - 108
        // 251, 0 -> value = -0*256 - 0 - 108 = -108
        let data = [251, 0, 17];
        assert_eq!(
            find_charstrings_offset(&data, 0, 3),
            Some((-108i32) as usize)
        );
    }

    #[test]
    fn test_find_charstrings_offset_three_byte() {
        // Three-byte (28): i16 big-endian
        // 28, 0x01, 0x00 -> 256
        let data = [28, 0x01, 0x00, 17];
        assert_eq!(find_charstrings_offset(&data, 0, 4), Some(256));
    }

    #[test]
    fn test_find_charstrings_offset_three_byte_negative() {
        // 28, 0xFF, 0x00 -> -256
        let data = [28, 0xFF, 0x00, 17];
        assert_eq!(
            find_charstrings_offset(&data, 0, 4),
            Some((-256i32) as usize)
        );
    }

    #[test]
    fn test_find_charstrings_offset_five_byte() {
        // Five-byte (29): i32 big-endian
        // 29, 0x00, 0x01, 0x00, 0x00 -> 65536
        let data = [29, 0x00, 0x01, 0x00, 0x00, 17];
        assert_eq!(find_charstrings_offset(&data, 0, 6), Some(65536));
    }

    #[test]
    fn test_find_charstrings_offset_five_byte_negative() {
        // 29, 0xFF, 0xFF, 0xFF, 0xFF -> -1
        let data = [29, 0xFF, 0xFF, 0xFF, 0xFF, 17];
        // -1 as usize will wrap around
        assert_eq!(find_charstrings_offset(&data, 0, 6), Some((-1i32) as usize));
    }

    #[test]
    fn test_find_charstrings_offset_no_operator_17() {
        // DICT without CharStrings operator
        let data = [139, 0]; // value 0, operator 0 (version)
        assert_eq!(find_charstrings_offset(&data, 0, 2), None);
    }

    #[test]
    fn test_find_charstrings_offset_multiple_operands() {
        // Multiple operands, only the last one before operator 17 is used
        // 139 -> 0, 140 -> 1, 246 -> 107, then operator 17
        let data = [139, 0, 140, 1, 246, 17];
        assert_eq!(find_charstrings_offset(&data, 0, 6), Some(107));
    }

    #[test]
    fn test_find_charstrings_offset_with_other_operators() {
        // DICT with other operators before CharStrings
        // operator 0 (version), operator 1 (Notice), then CharStrings
        let data = [
            139, 0, // version: 0
            139, 1, // Notice: 0
            139, 17, // CharStrings: 0
        ];
        assert_eq!(find_charstrings_offset(&data, 0, 6), Some(0));
    }

    #[test]
    fn test_find_charstrings_offset_two_byte_operator_long() {
        // Longer DICT with multiple two-byte operators and operands
        // Simulates a real CFF DICT with version, Notice, Copyright, Family, etc.
        //
        // DICT structure:
        //   operator 0 (version):     139 -> 0
        //   operator 1 (Notice):      140 -> 1
        //   operator 12 0 (Copyright): operands cleared, skip
        //   operator 2 (FullName):    141 -> 2
        //   operator 3 (FamilyName):  142 -> 3
        //   operator 12 1 (Weight):   operands cleared, skip
        //   operator 4 (FontBBox):    four operands 143,144,145,146 -> 4,5,6,7
        //   operator 12 5 (CharStrings): 247 100 -> (247-247)*256 + 100 + 108 = 208
        let data = [
            139, 0, // version: 0
            140, 1, // Notice: 1
            12, 0, // Copyright (two-byte op 12 0), clears operands
            141, 2, // FullName: 2
            142, 3, // FamilyName: 3
            12, 1, // Weight (two-byte op 12 1), clears operands
            143, 4, 144, 5, 145, 6, 146, 4, // FontBBox: 4 values
            247, 100, 17, // CharStrings offset: 208
        ];
        assert_eq!(find_charstrings_offset(&data, 0, data.len()), Some(208));
    }

    #[test]
    fn test_find_charstrings_offset_two_byte_operator_mixed() {
        // Mix of single-byte and two-byte operators with various number encodings
        //
        // DICT structure:
        //   operator 0:     139 -> 0
        //   operator 12 30: operands cleared (UnderlinePosition)
        //   operator 12 31: operands cleared (UnderlineThickness)
        //   operator 15:    28, 0x00, 0x64 -> 100 (Charset)
        //   operator 17:    28, 0x01, 0x2C -> 300 (CharStrings)
        let data = [
            139, 0, // version: 0
            12, 30, // UnderlinePosition (two-byte op)
            12, 31, // UnderlineThickness (two-byte op)
            28, 0x00, 0x64, 15, // Charset: 100
            28, 0x01, 0x2C, 17, // CharStrings: 300
        ];
        assert_eq!(find_charstrings_offset(&data, 0, data.len()), Some(300));
    }

    #[test]
    fn test_find_charstrings_offset_two_byte_operator_charstrings_not_found() {
        // Two-byte operators present but no operator 17 (CharStrings)
        // Should return None
        let data = [
            139, 0, // version: 0
            12, 0, // Copyright
            140, 1, // Notice: 1
            12, 1, // Weight
            141, 2, // FullName: 2
            12, 5, // CharStrings operator using two-byte form (12 5)
        ];
        // 12 5 is NOT operator 17, it's two-byte operator 5
        assert_eq!(find_charstrings_offset(&data, 0, data.len()), None);
    }

    #[test]
    fn test_find_charstrings_offset_with_bcd() {
        // BCD real number (30): nibbles until 0xf terminator
        // 30, 0x1f -> nibbles 1, f (terminator), then operator 17
        let data = [30, 0x1f, 17];
        assert_eq!(find_charstrings_offset(&data, 0, 3), None);
    }

    #[test]
    fn test_find_charstrings_offset_with_start_offset() {
        // Test with non-zero dict_start
        let data = [0x00, 0x00, 139, 17];
        assert_eq!(find_charstrings_offset(&data, 2, 4), Some(0));
    }

    #[test]
    fn test_find_charstrings_offset_truncated_two_byte_positive() {
        // Truncated two-byte positive number
        let data = [247]; // missing second byte
        assert_eq!(find_charstrings_offset(&data, 0, 1), None);
    }

    #[test]
    fn test_find_charstrings_offset_truncated_two_byte_negative() {
        // Truncated two-byte negative number
        let data = [251]; // missing second byte
        assert_eq!(find_charstrings_offset(&data, 0, 1), None);
    }

    #[test]
    fn test_find_charstrings_offset_truncated_three_byte() {
        // Truncated three-byte number
        let data = [28, 0x01]; // missing third byte
        assert_eq!(find_charstrings_offset(&data, 0, 2), None);
    }

    #[test]
    fn test_find_charstrings_offset_truncated_five_byte() {
        // Truncated five-byte number
        let data = [29, 0x00, 0x01, 0x00]; // missing fifth byte
        assert_eq!(find_charstrings_offset(&data, 0, 4), None);
    }

    #[test]
    fn test_find_charstrings_offset_truncated_two_byte_operator() {
        // Truncated two-byte operator
        let data = [12]; // missing second byte
        assert_eq!(find_charstrings_offset(&data, 0, 1), None);
    }

    #[test]
    fn test_rebuild_charstrings_index_no_truncation() {
        // CharStrings INDEX with 2 charstrings, no trailing data
        // CharString 1: 14 (endchar)
        // CharString 2: 139, 14 (0, endchar)
        let data = vec![
            0x00, 0x02, // count = 2
            0x01, // offSize = 1
            0x01, 0x02, 0x04, // offsets: 1, 2, 4
            14,   // charstring 1: endchar
            139, 14, // charstring 2: 0, endchar
        ];
        let mut data_clone = data.clone();
        // No truncation needed, should return false
        assert!(!rebuild_charstrings_index_at(&mut data_clone, 0, 0));
        // Data should remain unchanged
        assert_eq!(data_clone, data);
    }

    #[test]
    fn test_rebuild_charstrings_index_with_truncation() {
        // CharStrings INDEX with 1 charstring that has trailing data
        // CharString: 14, 139, 140 (endchar, trailing garbage)
        let mut data = vec![
            0x00, 0x01, // count = 1
            0x01, // offSize = 1
            0x01, 0x04, // offsets: 1, 4
            14,   // endchar
            139, 140, // trailing garbage
        ];
        assert!(rebuild_charstrings_index_at(&mut data, 0, 0));
        // Should be truncated: count=1, offSize=1, offsets 1,2, data=[14]
        assert_eq!(
            data[..5],
            [0x00, 0x01, 0x01, 0x01, 0x02] // count, offSize, offset[0], offset[1]
        );
        assert_eq!(data[5], 14); // endchar
    }

    #[test]
    fn test_rebuild_charstrings_index_multiple_truncation() {
        // CharStrings INDEX with 3 charstrings, all with trailing data
        let mut data = vec![
            0x00, 0x03, // count = 3
            0x01, // offSize = 1
            0x01, 0x03, 0x06, 0x0A, // offsets: 1, 3, 6, 10
            14, 139, // charstring 1: endchar + garbage
            14, 139, 140, // charstring 2: endchar + garbage
            14, 139, 140, 141, // charstring 3: endchar + garbage
        ];
        assert!(rebuild_charstrings_index_at(&mut data, 0, 0));
        // All 3 should be truncated to just endchar (14)
        // New offsets: 1, 2, 3, 4
        assert_eq!(data[..7], [0x00, 0x03, 0x01, 0x01, 0x02, 0x03, 0x04]);
        assert_eq!(data[7..10], [14, 14, 14]); // 3 endchars
    }

    #[test]
    fn test_rebuild_charstrings_index_with_offset() {
        // Data with header before CharStrings INDEX
        let mut data = vec![
            0xAB, 0xCD, // fake header
            0x00, 0x01, // count = 1
            0x01, // offSize = 1
            0x01, 0x04, // offsets: 1, 4
            14,   // endchar
            139, 140, // trailing garbage
        ];
        // cff_offset = 0, charstrings_offset = 2 (skip fake header)
        assert!(rebuild_charstrings_index_at(&mut data, 0, 2));
        // Header should remain
        assert_eq!(data[..2], [0xAB, 0xCD]);
    }

    #[test]
    fn test_rebuild_charstrings_index_empty_count() {
        // Invalid: count = 0
        let mut data = vec![
            0x00, 0x00, // count = 0
        ];
        assert!(!rebuild_charstrings_index_at(&mut data, 0, 0));
    }

    #[test]
    fn test_rebuild_charstrings_index_invalid_offsize() {
        // Invalid: offSize = 0
        let mut data = vec![
            0x00, 0x01, // count = 1
            0x00, // offSize = 0 (invalid)
            0x01, 0x02, 14,
        ];
        assert!(!rebuild_charstrings_index_at(&mut data, 0, 0));
    }

    #[test]
    fn test_rebuild_charstrings_index_truncated_header() {
        // Data too short for header
        let mut data = vec![0x00, 0x01]; // missing offSize
        assert!(!rebuild_charstrings_index_at(&mut data, 0, 0));
    }

    #[test]
    fn test_rebuild_charstrings_index_offsize_2() {
        // CharStrings INDEX with offSize = 2
        let mut data = vec![
            0x00, 0x02, // count = 2
            0x02, // offSize = 2
            0x00, 0x01, 0x00, 0x02, 0x00, 0x05, // offsets: 1, 2, 5
            14,   // charstring 1: endchar
            14, 139, 140, // charstring 2: endchar + garbage
        ];
        assert!(rebuild_charstrings_index_at(&mut data, 0, 0));
        // Charstring 1: unchanged (just endchar)
        // Charstring 2: truncated to 1 byte
        // New data: count=2, offSize=1 (smaller), offsets 1,2,3
        assert_eq!(data[..6], [0x00, 0x02, 0x01, 0x01, 0x02, 0x03]);
        assert_eq!(data[6..8], [14, 14]); // 2 endchars
    }

    #[test]
    fn test_rebuild_charstrings_index_large_offset() {
        // CharStrings INDEX with offSize = 2 and larger offsets
        let mut data = vec![
            0x00, 0x01, // count = 1
            0x02, // offSize = 2
            0x00, 0x01, 0x01, 0x00, // offsets: 1, 256
        ];
        // Add 255 bytes of charstring data
        data.extend(std::iter::repeat_n(14, 255)); // 255 endchars (only first is valid)

        // Truncate to just first byte (14)
        // New index should have offSize=1 since new total < 256
        assert!(rebuild_charstrings_index_at(&mut data, 0, 0));
        assert_eq!(data[..5], [0x00, 0x01, 0x01, 0x01, 0x02]);
    }
}
