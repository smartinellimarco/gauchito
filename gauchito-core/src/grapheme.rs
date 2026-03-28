use ropey::RopeSlice;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

/// Find the previous grapheme boundary before `char_idx`.
/// Returns 0 if already at the start.
// source: https://github.com/cessen/ropey/blob/master/examples/graphemes_step.rs
pub fn prev_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    let byte_idx = slice.char_to_byte(char_idx);
    let total_bytes = slice.len_bytes();
    let mut cursor = GraphemeCursor::new(byte_idx, total_bytes, true);
    let mut cur_byte = byte_idx;

    // Walk chunks backward until we find a boundary
    loop {
        let (chunk, chunk_byte_start, _, _) = if cur_byte == 0 {
            slice.chunk_at_byte(0)
        } else {
            slice.chunk_at_byte(cur_byte - 1)
        };

        match cursor.prev_boundary(chunk, chunk_byte_start) {
            Ok(None) => return 0,
            Ok(Some(byte_pos)) => return slice.byte_to_char(byte_pos),
            Err(GraphemeIncomplete::PrevChunk) => {
                cur_byte = chunk_byte_start;
            }
            Err(GraphemeIncomplete::PreContext(needed)) => {
                let (ctx_chunk, ctx_start, _, _) = slice.chunk_at_byte(needed.saturating_sub(1));
                cursor.provide_context(ctx_chunk, ctx_start);
            }
            _ => unreachable!(),
        }
    }
}

/// Find the next grapheme boundary after `char_idx`.
/// Returns `len_chars()` if already at the end.
pub fn next_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    let byte_idx = slice.char_to_byte(char_idx);
    let total_bytes = slice.len_bytes();
    let mut cursor = GraphemeCursor::new(byte_idx, total_bytes, true);
    let mut cur_byte = byte_idx;

    // Walk chunks forward until we find a boundary
    loop {
        let clamped = cur_byte.min(total_bytes.saturating_sub(1));
        let (chunk, chunk_byte_start, _, _) = if total_bytes == 0 {
            ("", 0, 0, 0)
        } else {
            slice.chunk_at_byte(clamped)
        };

        match cursor.next_boundary(chunk, chunk_byte_start) {
            Ok(None) => return slice.len_chars(),
            Ok(Some(byte_pos)) => return slice.byte_to_char(byte_pos),
            Err(GraphemeIncomplete::NextChunk) => {
                cur_byte = chunk_byte_start + chunk.len();
            }
            Err(GraphemeIncomplete::PreContext(needed)) => {
                let (ctx_chunk, ctx_start, _, _) = slice.chunk_at_byte(needed.saturating_sub(1));
                cursor.provide_context(ctx_chunk, ctx_start);
            }
            _ => unreachable!(),
        }
    }
}
