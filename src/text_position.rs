//! Conversions between byte offsets and (0-based line, UTF-16 column) positions,
//! the unit LSP and Monaco use.

pub fn utf16_position_to_byte_offset(source: &str, line: usize, column: usize) -> Option<usize> {
    let mut current_line = 0usize;
    let mut current_column = 0usize;

    for (idx, ch) in source.char_indices() {
        if current_line == line && current_column == column {
            return Some(idx);
        }
        if ch == '\n' {
            if current_line == line {
                return Some(idx);
            }
            current_line += 1;
            current_column = 0;
            continue;
        }
        current_column += ch.len_utf16();
        if current_line == line && current_column > column {
            return None;
        }
    }

    (current_line == line && current_column == column).then_some(source.len())
}

pub fn byte_offset_to_utf16_position(source: &str, target_offset: usize) -> (u32, u32) {
    let clamped = target_offset.min(source.len());
    let mut line = 0u32;
    let mut column = 0u32;
    for (idx, ch) in source.char_indices() {
        if idx >= clamped {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += ch.len_utf16() as u32;
        }
    }
    (line, column)
}
