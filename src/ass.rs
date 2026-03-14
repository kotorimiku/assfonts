use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::LazyLock,
};

const TAG_FN: &str = "fn";
const TAG_B: &str = "b";
const TAG_I: &str = "i";
const TAG_R: &str = "r";

static DEFAULT_STYLE_STATE: LazyLock<StyleState> = LazyLock::new(|| StyleState {
    font_name: "Default".to_string(),
    bold: 0,
    italic: 0,
});

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontRequest {
    pub font_name: String,
    pub bold: i32,
    pub italic: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AssAnalysis {
    pub font_requests: BTreeMap<FontRequest, HashSet<char>>,
}

#[derive(Debug, Clone)]
struct StyleState {
    font_name: String,
    bold: i32,
    italic: i32,
}

#[derive(Debug, Clone)]
struct StyleFormat {
    fields: Vec<String>,
    index_map: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct EventsFormat {
    fields: Vec<String>,
    index_map: HashMap<String, usize>,
}

pub fn analyze_ass(content: &str) -> AssAnalysis {
    let mut font_requests: BTreeMap<FontRequest, HashSet<char>> = BTreeMap::new();
    let mut section = "";
    let mut styles = std::collections::BTreeMap::<String, StyleState>::new();
    let mut style_format: Option<StyleFormat> = None;
    let mut events_format: Option<EventsFormat> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line;
            continue;
        }

        if (section.eq_ignore_ascii_case("[V4+ Styles]")
            || section.eq_ignore_ascii_case("[V4 Styles]"))
            && let Some(rest) = line.strip_prefix("Format:")
        {
            style_format = parse_style_format(rest);
            continue;
        }

        if section.eq_ignore_ascii_case("[Events]")
            && let Some(rest) = line.strip_prefix("Format:")
        {
            events_format = parse_events_format(rest);
            continue;
        }

        if (section.eq_ignore_ascii_case("[V4+ Styles]")
            || section.eq_ignore_ascii_case("[V4 Styles]"))
            && let Some(rest) = line.strip_prefix("Style:")
            && let Some((style_name, style_state)) = parse_style_line(rest, style_format.as_ref())
        {
            styles.insert(style_name, style_state);
        }

        if section.eq_ignore_ascii_case("[Events]")
            && line.starts_with("Dialogue:")
            && let Some(rest) = line.strip_prefix("Dialogue:")
        {
            let Some((style_name, text)) = parse_dialogue_line(rest, events_format.as_ref()) else {
                continue;
            };

            let state = styles.get(style_name).unwrap_or(&DEFAULT_STYLE_STATE);

            collect_dialogue_requests(text, state, &styles, &mut font_requests);
        }
    }

    AssAnalysis { font_requests }
}

fn parse_format_fields(rest: &str) -> Vec<String> {
    rest.split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(|x| x.to_ascii_lowercase())
        .collect()
}

fn build_index_map(fields: &[String]) -> HashMap<String, usize> {
    let mut index_map = HashMap::new();
    for (idx, field) in fields.iter().enumerate() {
        index_map.entry(field.clone()).or_insert(idx);
    }
    index_map
}

fn parse_style_format(rest: &str) -> Option<StyleFormat> {
    let fields = parse_format_fields(rest);
    let index_map = build_index_map(&fields);
    if !(index_map.contains_key("name") && index_map.contains_key("fontname")) {
        return None;
    }

    Some(StyleFormat { fields, index_map })
}

fn parse_events_format(rest: &str) -> Option<EventsFormat> {
    let fields = parse_format_fields(rest);
    let index_map = build_index_map(&fields);
    if !(index_map.contains_key("style") && index_map.contains_key("text")) {
        return None;
    }

    Some(EventsFormat { fields, index_map })
}

fn parse_style_line(
    rest: &str,
    style_format: Option<&StyleFormat>,
) -> Option<(String, StyleState)> {
    let columns: Vec<&str> = if let Some(fmt) = style_format {
        rest.splitn(fmt.fields.len(), ',').map(str::trim).collect()
    } else {
        rest.split(',').map(str::trim).collect()
    };

    let default_name_index = 0usize;
    let default_font_index = 1usize;
    let default_bold_index = Some(7usize);
    let default_italic_index = Some(8usize);

    let name_index = style_format
        .and_then(|fmt| fmt.index_map.get("name").copied())
        .unwrap_or(default_name_index);
    let font_index = style_format
        .and_then(|fmt| fmt.index_map.get("fontname").copied())
        .unwrap_or(default_font_index);
    let bold_index = style_format
        .and_then(|fmt| fmt.index_map.get("bold").copied())
        .or(default_bold_index);
    let italic_index = style_format
        .and_then(|fmt| fmt.index_map.get("italic").copied())
        .or(default_italic_index);

    let style_name = columns.get(name_index)?.to_string();
    let font_name = columns.get(font_index)?.to_string();
    let bold = bold_index
        .and_then(|x| columns.get(x))
        .and_then(|x| x.parse::<i32>().ok())
        .unwrap_or(0);
    let italic = italic_index
        .and_then(|x| columns.get(x))
        .and_then(|x| x.parse::<i32>().ok())
        .unwrap_or(0);

    Some((
        style_name,
        StyleState {
            font_name,
            bold,
            italic,
        },
    ))
}

fn parse_dialogue_line<'a>(
    rest: &'a str,
    events_format: Option<&EventsFormat>,
) -> Option<(&'a str, &'a str)> {
    let default_field_count = 10usize;
    let default_style_index = 3usize;
    let default_text_index = 9usize;

    let field_count = events_format
        .map(|fmt| fmt.fields.len())
        .unwrap_or(default_field_count);
    let style_index = events_format
        .and_then(|fmt| fmt.index_map.get("style").copied())
        .unwrap_or(default_style_index);
    let text_index = events_format
        .and_then(|fmt| fmt.index_map.get("text").copied())
        .unwrap_or(default_text_index);

    let columns: Vec<&str> = rest.splitn(field_count, ',').map(str::trim).collect();
    if columns.len() <= style_index || columns.len() <= text_index {
        return None;
    }

    Some((columns[style_index], columns[text_index]))
}

fn collect_dialogue_requests(
    text: &str,
    style_base: &StyleState,
    styles: &BTreeMap<String, StyleState>,
    sink: &mut BTreeMap<FontRequest, HashSet<char>>,
) {
    let mut current = style_base.clone();
    let mut in_override = false;
    let mut override_buf = String::new();

    for ch in text.chars() {
        if in_override {
            if ch == '}' {
                apply_override_state(&override_buf, &mut current, style_base, styles);
                override_buf.clear();
                in_override = false;
            } else {
                override_buf.push(ch);
            }
            continue;
        }

        if ch == '{' {
            in_override = true;
            continue;
        }

        if !ch.is_control() {
            let request = FontRequest {
                font_name: current.font_name.clone(),
                bold: normalize_bold(current.bold),
                italic: normalize_italic(current.italic),
            };
            sink.entry(request).or_default().insert(ch);
        }
    }
}

fn apply_override_state(
    code: &str,
    current: &mut StyleState,
    style_base: &StyleState,
    styles: &BTreeMap<String, StyleState>,
) {
    for_each_override_token(code, |tag, value| match tag {
        TAG_FN => {
            let name = value.trim();
            if !name.is_empty() {
                current.font_name = name.to_string();
            }
        }
        TAG_B => {
            if let Ok(parsed) = value.trim().parse::<i32>() {
                current.bold = parsed;
            }
        }
        TAG_I => {
            if let Ok(parsed) = value.trim().parse::<i32>() {
                current.italic = parsed;
            }
        }
        TAG_R => {
            let style_name = value.trim();
            if style_name.is_empty() {
                *current = style_base.clone();
            } else if let Some(style) = styles.get(style_name) {
                *current = style.clone();
            }
        }
        _ => {}
    });
}

fn for_each_override_token(code: &str, mut visit: impl FnMut(&str, &str)) {
    let mut cursor = 0usize;
    let bytes = code.as_bytes();

    while cursor < bytes.len() {
        if bytes[cursor] != b'\\' {
            cursor += 1;
            continue;
        }

        cursor += 1;
        let remainder = &code[cursor..];
        let tag = if remainder.starts_with(TAG_FN) {
            cursor += 2;
            TAG_FN
        } else if remainder.starts_with('b') {
            cursor += 1;
            TAG_B
        } else if remainder.starts_with('i') {
            cursor += 1;
            TAG_I
        } else if remainder.starts_with('r') {
            cursor += 1;
            TAG_R
        } else {
            while cursor < bytes.len() && bytes[cursor] != b'\\' {
                cursor += 1;
            }
            continue;
        };

        let value_start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'\\' {
            cursor += 1;
        }

        let value = &code[value_start..cursor];
        visit(tag, value);
    }
}

fn normalize_bold(value: i32) -> i32 {
    if value == -1 || value == 1 {
        return 700;
    }
    if value <= 0 {
        return 400;
    }
    value
}

fn normalize_italic(value: i32) -> bool {
    matches!(value, -1 | 1)
}

#[cfg(test)]
mod tests {
    use super::analyze_ass;

    #[test]
    fn parses_style_and_override_fonts() {
        let text = "[V4+ Styles]\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\nStyle: Fancy,Times New Roman,24,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,700,1,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n[Events]\nDialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\\fnRoboto}hello{\\rFancy}world";

        let analysis = analyze_ass(text);
        assert!(
            analysis
                .font_requests
                .keys()
                .any(|x| x.font_name == "Roboto")
        );
    }

    #[test]
    fn handles_mixed_override_order_and_reset() {
        let text = "[V4+ Styles]\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\nStyle: Fancy,Times New Roman,24,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,700,1,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n[Events]\nDialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,A{\\b700\\fnRoboto}B{\\i1}C{\\rFancy}D{\\r}E";

        let analysis = analyze_ass(text);

        assert!(analysis.font_requests.keys().any(|request| {
            request.font_name == "Roboto" && request.bold == 700 && !request.italic
        }));
        assert!(analysis.font_requests.keys().any(|request| {
            request.font_name == "Roboto" && request.bold == 700 && request.italic
        }));
        assert!(analysis.font_requests.keys().any(|request| {
            request.font_name == "Times New Roman" && request.bold == 700 && request.italic
        }));
        assert!(analysis.font_requests.keys().any(|request| {
            request.font_name == "Arial" && request.bold == 400 && !request.italic
        }));
    }

    #[test]
    fn parses_with_format_vec_and_map() {
        let text = "[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic\nStyle: Default,Arial,20,&H00FFFFFF,&H00112233,&H00000000,&H00000000,700,1\n[Events]\nFormat: Layer, Start, End, Name, Style, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:00.00,0:00:02.00,,Default,0,0,0,,A";

        let analysis = analyze_ass(text);

        assert!(analysis.font_requests.keys().any(|request| {
            request.font_name == "Arial" && request.bold == 700 && request.italic
        }));
    }
}
