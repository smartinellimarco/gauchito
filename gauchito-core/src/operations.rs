// TODO: Implement clipboard integration
// TODO: can treesitter do this (indent)?
// TODO: this should work on slices (find next)
// TODO: Implement file saving
// TODO: Implement clipboard integration
// TODO: handle eol and tabs for each OS (replace \t) maybe editorconfig
// should be a field of the buffer
// TODO: ALL movements should use graphemes as base to keep the rest
// of implementations char based
use crate::context::Context;
use crate::edit::Edit;
use crate::query::{NavigationTarget, Query};
use crate::textobjects::textobject::{Selection, TextObjectKind};

#[derive(Debug, Clone)]
pub enum OperationResult {
    Continue,
    SwitchMode(String),
    Exit,
}

pub trait Operation: std::fmt::Debug {
    fn execute(&self, ctx: &mut Context) -> OperationResult;
    fn name(&self) -> &'static str;
}

// ==== CURSOR MOVEMENT OPERATIONS (using Query system) ====

#[derive(Debug, Clone)]
pub struct MoveLeft;
impl Operation for MoveLeft {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::PrevGrapheme) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_left"
    }
}

#[derive(Debug, Clone)]
pub struct MoveRight;
impl Operation for MoveRight {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::NextGrapheme) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_right"
    }
}

#[derive(Debug, Clone)]
pub struct MoveWordForward;
impl Operation for MoveWordForward {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::NextWord) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_word_forward"
    }
}

#[derive(Debug, Clone)]
pub struct MoveWordBackward;
impl Operation for MoveWordBackward {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::PrevWord) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_word_backward"
    }
}

#[derive(Debug, Clone)]
pub struct MoveBigWordForward;
impl Operation for MoveBigWordForward {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::NextBigWord) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_big_word_forward"
    }
}

#[derive(Debug, Clone)]
pub struct MoveBigWordBackward;
impl Operation for MoveBigWordBackward {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::PrevBigWord) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_big_word_backward"
    }
}

#[derive(Debug, Clone)]
pub struct MoveMatchingBracket;
impl Operation for MoveMatchingBracket {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::MatchingBracket) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_matching_bracket"
    }
}

#[derive(Debug, Clone)]
pub struct MoveParagraphForward;
impl Operation for MoveParagraphForward {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::NextParagraph) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_paragraph_forward"
    }
}

#[derive(Debug, Clone)]
pub struct MoveParagraphBackward;
impl Operation for MoveParagraphBackward {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(new_pos) = ctx.navigate(NavigationTarget::PrevParagraph) {
            ctx.selection_mut().cursor_to(new_pos);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "move_paragraph_backward"
    }
}

// ==== SELECTION OPERATIONS (using Query system) ====

#[derive(Debug, Clone)]
pub struct SelectLeft;
impl Operation for SelectLeft {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        let current_head = ctx.selection().head;
        if let Some(new_head) = ctx
            .query(Query::Navigate {
                from: current_head,
                direction: NavigationTarget::PrevGrapheme,
            })
            .first()
        {
            ctx.selection_mut()
                .set_range(ctx.selection().anchor, new_head.start);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "select_left"
    }
}

#[derive(Debug, Clone)]
pub struct SelectRight;
impl Operation for SelectRight {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        let current_head = ctx.selection().head;
        if let Some(new_head) = ctx
            .query(Query::Navigate {
                from: current_head,
                direction: NavigationTarget::NextGrapheme,
            })
            .first()
        {
            ctx.selection_mut()
                .set_range(ctx.selection().anchor, new_head.start);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "select_right"
    }
}

#[derive(Debug, Clone)]
pub struct SelectWord;
impl Operation for SelectWord {
    fn execute(&self, ctx: &mut Context) -> OperationResult {
        if let Some(range) = ctx.find_text_object(TextObjectKind::Word, Selection::Around) {
            ctx.selection_mut().set_range(range.start, range.end);
        }
        OperationResult::Continue
    }
    fn name(&self) -> &'static str {
        "select_word"
    }
}
