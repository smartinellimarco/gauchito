use super::{SelectionMode, TextObjectMatcher, TextSource};
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

/// TreeSitter-based text object manager
#[derive(Debug)]
pub struct TreeSitterEngine {
    parser: Parser,
    tree: Option<Tree>,
    language: Option<Language>,
    queries: HashMap<String, Query>,
    query_dir: PathBuf,
}

impl TreeSitterEngine {
    pub fn new(query_dir: PathBuf) -> Self {
        Self {
            parser: Parser::new(),
            tree: None,
            language: None,
            queries: HashMap::new(),
            query_dir,
        }
    }

    /// Set the language for parsing
    pub fn set_language(&mut self, language: Language) -> Result<(), Box<dyn std::error::Error>> {
        self.parser.set_language(language)?;
        self.language = Some(language);
        self.tree = None;
        self.queries.clear();
        Ok(())
    }

    /// Parse or reparse the buffer
    pub fn parse(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.language.is_none() {
            return Err("No language set".into());
        }

        self.tree = self.parser.parse(text, None);
        Ok(())
    }

    /// Update tree with edit
    pub fn update_for_edit(
        &mut self,
        text: &str,
        edit: &tree_sitter::InputEdit,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tree) = &mut self.tree {
            tree.edit(edit);
            self.tree = self.parser.parse(text, Some(tree));
        }
        Ok(())
    }

    /// Load a query from file
    pub fn load_query(
        &mut self,
        name: &str,
        language_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let language = self.language.ok_or("No language set")?;

        // Load query file from: queries/{language_name}/textobjects.scm
        let query_path = self.query_dir.join(language_name).join("textobjects.scm");

        let query_source = std::fs::read_to_string(&query_path)
            .map_err(|e| format!("Failed to read query file {:?}: {}", query_path, e))?;

        let query = Query::new(language, &query_source)?;
        self.queries.insert(name.to_string(), query);

        Ok(())
    }

    /// Find nodes matching a capture name
    pub fn find_nodes(&self, capture_name: &str, range: Option<Range<usize>>) -> Vec<Range<usize>> {
        let tree = match &self.tree {
            Some(t) => t,
            None => return vec![],
        };

        let query = match self.queries.get("textobjects") {
            Some(q) => q,
            None => return vec![],
        };

        let mut cursor = QueryCursor::new();
        let root_node = tree.root_node();

        // Set range if provided
        if let Some(range) = range {
            cursor.set_byte_range(range);
        }

        let text_callback = |_node: tree_sitter::Node| -> &[u8] { &[] };

        let mut results = Vec::new();

        for capture in cursor.captures(query, root_node, text_callback) {
            for cap in capture.0.captures {
                if let Some(name) = query.capture_names().get(cap.index as usize) {
                    if name == capture_name {
                        let node = cap.node;
                        results.push(node.start_byte()..node.end_byte());
                    }
                }
            }
        }

        results
    }

    /// Find the node at a specific position
    pub fn node_at(&self, pos: usize, capture_name: &str) -> Option<Range<usize>> {
        let ranges = self.find_nodes(capture_name, Some(pos..pos + 1));

        // Find the smallest node containing pos
        ranges
            .into_iter()
            .filter(|r| r.start <= pos && pos < r.end)
            .min_by_key(|r| r.end - r.start)
    }

    /// Find next node after position
    pub fn node_next(&self, pos: usize, capture_name: &str) -> Option<Range<usize>> {
        let ranges = self.find_nodes(capture_name, None);

        ranges
            .into_iter()
            .filter(|r| r.start > pos)
            .min_by_key(|r| r.start)
    }

    /// Find previous node before position
    pub fn node_prev(&self, pos: usize, capture_name: &str) -> Option<Range<usize>> {
        let ranges = self.find_nodes(capture_name, None);

        ranges
            .into_iter()
            .filter(|r| r.end < pos)
            .max_by_key(|r| r.end)
    }
}

/// Text object matcher using TreeSitter queries
#[derive(Debug)]
pub struct TreeSitterMatcher {
    engine: Arc<RwLock<TreeSitterEngine>>,
    capture_name: String,
}

impl TreeSitterMatcher {
    pub fn new(engine: Arc<RwLock<TreeSitterEngine>>, capture_name: String) -> Self {
        Self {
            engine,
            capture_name,
        }
    }

    // Factory methods for common text objects
    pub fn function(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "function.outer".to_string())
    }

    pub fn function_inner(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "function.inner".to_string())
    }

    pub fn class(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "class.outer".to_string())
    }

    pub fn class_inner(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "class.inner".to_string())
    }

    pub fn parameter(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "parameter.outer".to_string())
    }

    pub fn parameter_inner(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "parameter.inner".to_string())
    }

    pub fn comment(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "comment.outer".to_string())
    }

    pub fn block(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "block.outer".to_string())
    }

    pub fn block_inner(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "block.inner".to_string())
    }

    pub fn conditional(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "conditional.outer".to_string())
    }

    pub fn loop_obj(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "loop.outer".to_string())
    }

    pub fn call(engine: Arc<RwLock<TreeSitterEngine>>) -> Self {
        Self::new(engine, "call.outer".to_string())
    }

    /// Convert byte range to char range
    fn byte_to_char_range(
        &self,
        buffer: &dyn TextSource,
        byte_range: Range<usize>,
    ) -> Range<usize> {
        // This is a simplified conversion - in a real implementation,
        // you'd need to properly track byte-to-char mappings
        // For now, assuming they're the same (works for ASCII)
        byte_range
    }
}

impl TextObjectMatcher for TreeSitterMatcher {
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let engine = self.engine.read().ok()?;

        // Determine which capture to use based on mode
        let capture_name = match mode {
            SelectionMode::Inside => {
                // Try to use .inner variant if available
                if self.capture_name.ends_with(".outer") {
                    self.capture_name.replace(".outer", ".inner")
                } else if !self.capture_name.contains(".") {
                    format!("{}.inner", self.capture_name)
                } else {
                    self.capture_name.clone()
                }
            }
            SelectionMode::Around => {
                // Use .outer variant
                if self.capture_name.ends_with(".inner") {
                    self.capture_name.replace(".inner", ".outer")
                } else if !self.capture_name.contains(".") {
                    format!("{}.outer", self.capture_name)
                } else {
                    self.capture_name.clone()
                }
            }
        };

        let byte_range = engine.node_at(pos, &capture_name)?;
        Some(self.byte_to_char_range(buffer, byte_range))
    }

    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let engine = self.engine.read().ok()?;

        let capture_name = match mode {
            SelectionMode::Inside => {
                if self.capture_name.ends_with(".outer") {
                    self.capture_name.replace(".outer", ".inner")
                } else {
                    self.capture_name.clone()
                }
            }
            SelectionMode::Around => self.capture_name.clone(),
        };

        let byte_range = engine.node_next(pos, &capture_name)?;
        Some(self.byte_to_char_range(buffer, byte_range))
    }

    fn find_prev(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let engine = self.engine.read().ok()?;

        let capture_name = match mode {
            SelectionMode::Inside => {
                if self.capture_name.ends_with(".outer") {
                    self.capture_name.replace(".outer", ".inner")
                } else {
                    self.capture_name.clone()
                }
            }
            SelectionMode::Around => self.capture_name.clone(),
        };

        let byte_range = engine.node_prev(pos, &capture_name)?;
        Some(self.byte_to_char_range(buffer, byte_range))
    }
}
