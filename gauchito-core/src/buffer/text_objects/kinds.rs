#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextObjectKind {
    // TODO: grapheme
    // Basic text objects
    Word,
    BigWord,
    Sentence,
    Paragraph,

    // Delimiter pairs
    Parentheses,   // ()
    Brackets,      // []
    Braces,        // {}
    AngleBrackets, // <>
    SingleQuotes,  // ''
    DoubleQuotes,  // ""
    Backticks,     // ``

    // Regex-based
    Url,
    Email,
    Number,
    HexColor,

    // Tree-sitter based
    Function,
    Class,
    Parameter,
    Argument,
    Comment,
    String,
    Type,
    Import,
    Return,
    Conditional,
    Loop,
    Block,
    Call,
    Assignment,
}

impl TextObjectKind {
    pub fn requires_treesitter(&self) -> bool {
        matches!(
            self,
            TextObjectKind::Function
                | TextObjectKind::Class
                | TextObjectKind::Parameter
                | TextObjectKind::Argument
                | TextObjectKind::Comment
                | TextObjectKind::String
                | TextObjectKind::Type
                | TextObjectKind::Import
                | TextObjectKind::Return
                | TextObjectKind::Conditional
                | TextObjectKind::Loop
                | TextObjectKind::Block
                | TextObjectKind::Call
                | TextObjectKind::Assignment
        )
    }
}
