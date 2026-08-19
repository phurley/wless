use crate::document::Document;
use regex::bytes::{Regex, RegexBuilder};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

impl Direction {
    pub fn opposite(self) -> Self {
        match self {
            Direction::Forward => Direction::Backward,
            Direction::Backward => Direction::Forward,
        }
    }

    pub fn prompt_char(self) -> char {
        match self {
            Direction::Forward => '/',
            Direction::Backward => '?',
        }
    }
}

#[derive(Debug, Clone)]
pub struct Match {
    pub line: usize,
    pub range: Range<usize>,
}

pub fn compile(pattern: &str, ignore_case: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
}

/// Search forward starting just after `from_line`. The full line index
/// always exists (files fit in memory), so this is a plain scan -- no
/// incremental index extension needed even while follow mode is appending.
pub fn search_forward(doc: &Document, re: &Regex, from_line: usize) -> Option<Match> {
    ((from_line + 1)..doc.line_count()).find_map(|i| {
        re.find(doc.line_bytes(i)).map(|m| Match {
            line: i,
            range: m.range(),
        })
    })
}

/// Search backward starting just before `from_line`.
pub fn search_backward(doc: &Document, re: &Regex, from_line: usize) -> Option<Match> {
    (0..from_line).rev().find_map(|i| {
        re.find(doc.line_bytes(i)).map(|m| Match {
            line: i,
            range: m.range(),
        })
    })
}

/// All match byte-ranges within one logical line, for highlighting.
pub fn find_all_in_line(re: &Regex, line_bytes: &[u8]) -> Vec<Range<usize>> {
    re.find_iter(line_bytes).map(|m| m.range()).collect()
}
