use ropey::RopeSlice;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

pub fn prev_grapheme_boundary(slice: &RopeSlice, byte_idx: usize) -> usize {
    let mut cursor = GraphemeCursor::new(byte_idx, slice.len(), true);
    let mut cur_byte = byte_idx;

    loop {
        let (chunk, chunk_start) = slice.chunk(cur_byte.saturating_sub(1));

        match cursor.prev_boundary(chunk, chunk_start) {
            Ok(None) => return 0,
            Ok(Some(byte_pos)) => return byte_pos,
            Err(GraphemeIncomplete::PrevChunk) => cur_byte = chunk_start,
            Err(GraphemeIncomplete::PreContext(needed)) => {
                let (ctx_chunk, ctx_start) = slice.chunk(needed.saturating_sub(1));
                cursor.provide_context(ctx_chunk, ctx_start);
            }
            _ => unreachable!(),
        }
    }
}

pub fn next_grapheme_boundary(slice: &RopeSlice, byte_idx: usize) -> usize {
    let mut cursor = GraphemeCursor::new(byte_idx, slice.len(), true);
    let mut cur_byte = byte_idx;

    loop {
        let (chunk, chunk_start) = slice.chunk(cur_byte);

        match cursor.next_boundary(chunk, chunk_start) {
            Ok(None) => return slice.len(),
            Ok(Some(byte_pos)) => return byte_pos,
            Err(GraphemeIncomplete::NextChunk) => cur_byte = chunk_start + chunk.len(),
            Err(GraphemeIncomplete::PreContext(needed)) => {
                let (ctx_chunk, ctx_start) = slice.chunk(needed.saturating_sub(1));
                cursor.provide_context(ctx_chunk, ctx_start);
            }
            _ => unreachable!(),
        }
    }
}
