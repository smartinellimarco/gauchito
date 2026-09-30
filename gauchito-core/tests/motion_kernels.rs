use gauchito_core::movement::{
    CharClass, char_class, last_navigable_line, move_first_non_whitespace,
    move_grapheme, move_line_end, move_line_start, move_to_line, move_vertical,
    position_at, skip_class_backward, skip_class_forward, visual_column,
};
use ropey::Rope;

fn rope(s: &str) -> Rope {
    Rope::from_str(s)
}

#[test]
fn classify_chars() {
    assert_eq!(char_class('\n'), CharClass::Eol);
    assert_eq!(char_class('\r'), CharClass::Eol);
    assert_eq!(char_class(' '), CharClass::Whitespace);
    assert_eq!(char_class('\t'), CharClass::Whitespace);
    assert_eq!(char_class('a'), CharClass::Word);
    assert_eq!(char_class('_'), CharClass::Word);
    assert_eq!(char_class('9'), CharClass::Word);
    assert_eq!(char_class('.'), CharClass::Punct);
    assert_eq!(char_class('('), CharClass::Punct);
}

#[test]
fn h_move_clamps_at_start() {
    let text = rope("hello");
    assert_eq!(move_grapheme(&text.slice(..), 0, -1), 0);
}

#[test]
fn l_move_clamps_at_end() {
    let text = rope("hi");
    assert_eq!(move_grapheme(&text.slice(..), 2, 5), 2);
}

#[test]
fn j_moves_down_preserving_col() {
    let text = rope("abc\nde\nfghij");
    assert_eq!(move_vertical(&text.slice(..), 2, 1, None), 5);
}

#[test]
fn k_moves_up_preserving_col() {
    let text = rope("abcde\nfg");
    assert_eq!(move_vertical(&text.slice(..), 7, -1, None), 1);
}

#[test]
fn sticky_col_remembers_wider_col() {
    let text = rope("abcde\nfg\nhijklm");
    let p1 = move_vertical(&text.slice(..), 4, 1, None);
    assert_eq!(p1, 7);
    let p2 = move_vertical(&text.slice(..), p1, 1, Some(4));
    assert_eq!(p2, 13);
}

#[test]
fn k_at_first_line_stays_put() {
    let text = rope("hello\nworld");
    assert_eq!(move_vertical(&text.slice(..), 2, -1, None), 2);
}

#[test]
fn j_at_last_line_stays_put() {
    let text = rope("hello\nworld");
    assert_eq!(move_vertical(&text.slice(..), 7, 1, None), 7);
}

#[test]
fn j_descends_onto_trailing_newline_line() {
    // Trailing `\n` creates a navigable empty line — vim-like.
    // From "def" (offset 4), `j` lands on the empty line below.
    let text = rope("abc\ndef\n");
    let dest = move_vertical(&text.slice(..), 4, 1, None);
    // The empty trailing line has bol = 8 (just past the final `\n`).
    assert_eq!(dest, 8);
}

#[test]
fn j_from_trailing_line_stays_put() {
    // Already on the trailing empty line — j is a no-op.
    let text = rope("abc\n");
    // bol of the trailing empty line is text.len() (4).
    let pos = text.len();
    assert_eq!(move_vertical(&text.slice(..), pos, 1, None), pos);
}

#[test]
fn last_navigable_line_handles_empty_and_trailing_newline() {
    // Empty buffer: line 0 is the (empty) only line.
    assert_eq!(last_navigable_line(&rope("").slice(..)), 0);
    // No trailing newline: last line index is len_lines() - 1.
    assert_eq!(last_navigable_line(&rope("abc").slice(..)), 0);
    // Trailing newline creates an empty line below.
    assert_eq!(last_navigable_line(&rope("abc\n").slice(..)), 1);
    assert_eq!(last_navigable_line(&rope("abc\ndef").slice(..)), 1);
    assert_eq!(last_navigable_line(&rope("abc\ndef\n").slice(..)), 2);
    // Two trailing newlines: one empty middle line + one empty last.
    assert_eq!(last_navigable_line(&rope("abc\n\n").slice(..)), 2);
}

#[test]
fn skip_class_fwd_word() {
    assert_eq!(skip_class_forward(&rope("hello.world").slice(..), 0), 5);
}

#[test]
fn skip_class_fwd_punct() {
    assert_eq!(skip_class_forward(&rope("...abc").slice(..), 0), 3);
}

#[test]
fn skip_class_fwd_at_end() {
    assert_eq!(skip_class_forward(&rope("hi").slice(..), 2), 2);
}

#[test]
fn skip_class_bwd_word() {
    assert_eq!(skip_class_backward(&rope("hello.world").slice(..), 11), 6);
}

#[test]
fn skip_class_bwd_punct() {
    assert_eq!(skip_class_backward(&rope("abc...").slice(..), 6), 3);
}

#[test]
fn skip_class_bwd_at_start() {
    assert_eq!(skip_class_backward(&rope("hi").slice(..), 0), 0);
}

#[test]
fn skip_class_fwd_whitespace() {
    assert_eq!(skip_class_forward(&rope("   abc").slice(..), 0), 3);
}

#[test]
fn skip_class_fwd_whitespace_stops_at_eol() {
    assert_eq!(skip_class_forward(&rope("  \nabc").slice(..), 0), 2);
}

#[test]
fn skip_class_bwd_whitespace() {
    assert_eq!(skip_class_backward(&rope("abc   def").slice(..), 6), 3);
}

#[test]
fn skip_class_bwd_whitespace_stops_at_eol() {
    assert_eq!(skip_class_backward(&rope("abc\n  def").slice(..), 6), 4);
}

#[test]
fn move_to_line_basic() {
    assert_eq!(move_to_line(&rope("abc\ndef\nghi").slice(..), 1), 4);
}

#[test]
fn move_to_line_clamps() {
    assert_eq!(move_to_line(&rope("abc\ndef").slice(..), 100), 4);
}

#[test]
fn zero_goes_to_line_start() {
    assert_eq!(move_line_start(&rope("abc\ndef").slice(..), 5), 4);
}

#[test]
fn line_end_is_last_visible() {
    assert_eq!(move_line_end(&rope("abc\ndef").slice(..), 1), 2);
}

#[test]
fn line_end_on_last_line_no_newline() {
    assert_eq!(move_line_end(&rope("abc").slice(..), 0), 2);
}

#[test]
fn line_end_empty_line() {
    assert_eq!(move_line_end(&rope("abc\n\ndef").slice(..), 4), 4);
}

#[test]
fn first_non_ws_skips_indent() {
    assert_eq!(
        move_first_non_whitespace(&rope("  \thello").slice(..), 0),
        3
    );
}

#[test]
fn first_non_ws_no_indent() {
    assert_eq!(move_first_non_whitespace(&rope("hello").slice(..), 0), 0);
}

#[test]
fn grapheme_moves_step_over_multibyte_clusters() {
    let text = rope("añ👍🏽b");
    let t = text.slice(..);
    assert_eq!(move_grapheme(&t, 1, 1), 3);
    assert_eq!(move_grapheme(&t, 3, 1), 11);
    assert_eq!(move_grapheme(&t, 11, -1), 3);
    assert_eq!(move_grapheme(&t, 11, -2), 1);
}

#[test]
fn grapheme_moves_on_empty_rope_stay_at_zero() {
    let text = rope("");
    assert_eq!(move_grapheme(&text.slice(..), 0, 1), 0);
    assert_eq!(move_grapheme(&text.slice(..), 0, -1), 0);
}

#[test]
fn class_skips_count_bytes() {
    let text = rope("ñandú ok");
    let t = text.slice(..);
    assert_eq!(skip_class_forward(&t, 0), 7);
    assert_eq!(skip_class_backward(&t, 7), 0);
}

#[test]
fn columns_count_chars_and_positions_count_bytes() {
    let text = rope("ññññ
abcd");
    let t = text.slice(..);
    assert_eq!(visual_column(&t, 6), 3);
    assert_eq!(position_at(&t, 0, 2), 4);
    assert_eq!(move_vertical(&t, 11, -1, None), 4);
    assert_eq!(move_line_end(&t, 0), 6);
}
