//! Filename normalization, type filtering and stable result ordering.
use super::PAGE_SIZE;
use anywhere_platform::FileSearchHit;
use anywhere_protocol::FileSearchFilter;
use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

pub fn normalize(text: &str) -> String {
    text.nfc().case_fold().nfc().collect()
}

#[derive(Clone)]
pub struct NameQuery {
    words: Vec<String>,
    joined: String,
    filter: FileSearchFilter,
}
impl NameQuery {
    pub fn new(text: &str, filter: FileSearchFilter) -> Self {
        let joined = normalize(text.trim());
        Self {
            words: joined.split_whitespace().map(str::to_owned).collect(),
            joined,
            filter,
        }
    }
    pub fn rank(&self, path: &Path, is_directory: bool) -> Option<RankedHit> {
        let name = path.file_name()?.to_str()?;
        let normalized = normalize(name);
        if !self.filter.accepts(name, is_directory) {
            return None;
        }
        let literal = self.words.iter().all(|w| normalized.contains(w));
        if !literal && !crate::core::search::pinyin::matches(&normalized, &self.words) {
            return None;
        }
        let score = if !literal {
            500
        } else if normalized == self.joined {
            1000
        } else if normalized.starts_with(&self.joined) {
            900
        } else {
            700
        };
        Some(RankedHit {
            path: path.to_owned(),
            is_directory,
            normalized,
            score,
        })
    }
}

trait FilterName {
    fn accepts(self, name: &str, directory: bool) -> bool;
}
impl FilterName for FileSearchFilter {
    fn accepts(self, name: &str, directory: bool) -> bool {
        super::super::file_types::accepts(self, name, directory)
    }
}

#[derive(Debug, Clone)]
pub struct RankedHit {
    pub path: PathBuf,
    pub is_directory: bool,
    pub normalized: String,
    pub score: u16,
}
impl RankedHit {
    pub fn compare(&self, other: &Self) -> Ordering {
        other
            .score
            .cmp(&self.score)
            .then_with(|| self.normalized.cmp(&other.normalized))
            .then_with(|| self.path.cmp(&other.path))
    }
    pub fn hit(&self) -> FileSearchHit {
        FileSearchHit {
            path: self.path.clone(),
            is_directory: self.is_directory,
        }
    }
}
pub(super) fn insert_top(top: &mut Vec<RankedHit>, hit: RankedHit) {
    let at = top
        .binary_search_by(|old| old.compare(&hit))
        .unwrap_or_else(|at| at);
    if at >= PAGE_SIZE {
        return;
    }
    top.insert(at, hit);
    if top.len() > PAGE_SIZE {
        top.pop();
    }
}

pub(super) fn insert_before(top: &mut Vec<RankedHit>, hit: RankedHit) {
    let at = top
        .binary_search_by(|old| hit.compare(old))
        .unwrap_or_else(|at| at);
    if at >= PAGE_SIZE {
        return;
    }
    top.insert(at, hit);
    if top.len() > PAGE_SIZE {
        top.pop();
    }
}
