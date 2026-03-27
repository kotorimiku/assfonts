#[derive(Debug, Clone)]
pub struct EmbeddedFont {
    pub font_name: String,
    pub bytes: Vec<u8>,
}

pub fn uuencode(input: &[u8], insert_linebreaks: bool) -> String {
    let size = input.len();
    let mut output = String::with_capacity((size * 4).div_ceil(3) + size / 80 * 2);
    let mut written = 0usize;

    let mut pos = 0usize;
    while pos < size {
        let remaining = size - pos;
        let src0 = input[pos];
        let src1 = if remaining > 1 { input[pos + 1] } else { 0 };
        let src2 = if remaining > 2 { input[pos + 2] } else { 0 };

        let dst = [
            src0 >> 2,
            ((src0 & 0x3) << 4) | ((src1 & 0xF0) >> 4),
            ((src1 & 0xF) << 2) | ((src2 & 0xC0) >> 6),
            src2 & 0x3F,
        ];

        let write_count = usize::min(remaining + 1, 4);
        for value in dst.iter().take(write_count) {
            output.push((value + 33) as char);
            written += 1;
            if insert_linebreaks && written == 80 && pos + 3 < size {
                written = 0;
                output.push('\n');
            }
        }

        pos += 3;
    }

    output
}

pub fn render_ass_with_fonts(original: &str, fonts: &[EmbeddedFont]) -> String {
    if fonts.is_empty() {
        return original.to_string();
    }

    let lines: Vec<String> = original.lines().map(|x| x.to_string()).collect();
    let cleaned = remove_existing_fonts_section(&lines);
    let mut output = String::new();

    for line in cleaned.iter() {
        output.push_str(line);
        output.push('\n');
    }

    output.push_str("\n[Fonts]\n");
    for font in fonts {
        output.push_str("fontname: ");
        output.push_str(&font.font_name);
        output.push('\n');
        output.push_str(&uuencode(&font.bytes, true));
        output.push('\n');
    }

    output
}

fn remove_existing_fonts_section(lines: &[String]) -> Vec<String> {
    let mut output = Vec::with_capacity(lines.len());
    let mut skipping = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if trimmed.eq_ignore_ascii_case("[fonts]") {
                skipping = true;
                continue;
            }
            if skipping {
                skipping = false;
            }
        }

        if !skipping {
            output.push(line.clone());
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{EmbeddedFont, render_ass_with_fonts, uuencode};

    #[test]
    fn uuencode_matches_known_result() {
        let encoded = uuencode(b"abc", false);
        assert_eq!(encoded, "97*D");
    }

    #[test]
    fn inserts_fonts_section_at_end() {
        let source = "[Script Info]\nTitle: demo\n[Events]\nDialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,hi";
        let fonts = vec![EmbeddedFont {
            font_name: "demo.ttf_0.ttf".to_string(),
            bytes: b"abc".to_vec(),
        }];

        let rendered = render_ass_with_fonts(source, &fonts);
        assert!(rendered.ends_with("\n[Fonts]\nfontname: demo.ttf_0.ttf\n97*D\n"));
        assert!(rendered.contains("[Events]"));
    }
}
