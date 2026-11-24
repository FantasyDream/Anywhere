//! Keep file search results in their immutable Core snapshot. Navigation needs
//! an ID and availability. Only a visible or explicitly requested row needs text.
use super::{LauncherRow, Locale};
use anywhere_protocol::{CandidateKind, CandidatePresentation};
use std::{borrow::Cow, collections::HashSet, sync::Arc};

#[derive(Clone, Debug)]
pub struct LauncherRows(Storage);

#[derive(Clone, Debug)]
enum Storage {
    Projected(Vec<LauncherRow>),
    Files {
        candidates: Arc<Vec<CandidatePresentation>>,
        indices: Vec<usize>,
        locale: Locale,
    },
}

impl LauncherRows {
    pub(super) fn files(
        candidates: &Arc<Vec<CandidatePresentation>>,
        locale: Locale,
    ) -> Option<Self> {
        let mut seen = HashSet::with_capacity(candidates.len());
        let mut indices = Vec::with_capacity(candidates.len());
        for (index, row) in candidates.iter().enumerate() {
            if row.kind != CandidateKind::File || row.is_menu {
                return None;
            }
            if row.availability.is_visible() && seen.insert(row.id.as_str()) {
                indices.push(index);
            }
        }
        Some(Self(Storage::Files {
            candidates: candidates.clone(),
            indices,
            locale,
        }))
    }

    pub fn len(&self) -> usize {
        match &self.0 {
            Storage::Projected(rows) => rows.len(),
            Storage::Files { indices, .. } => indices.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn clear(&mut self) {
        *self = Vec::new().into();
    }

    pub fn get(&self, index: usize) -> Option<Cow<'_, LauncherRow>> {
        match &self.0 {
            Storage::Projected(rows) => rows.get(index).map(Cow::Borrowed),
            Storage::Files {
                candidates,
                indices,
                locale,
            } => {
                let candidate = &candidates[*indices.get(index)?];
                Some(Cow::Owned(LauncherRow::from_protocol(
                    candidate, None, *locale,
                )))
            }
        }
    }

    pub(super) fn id(&self, index: usize) -> Option<&str> {
        match &self.0 {
            Storage::Projected(rows) => rows.get(index).map(|row| row.id.as_str()),
            Storage::Files {
                candidates,
                indices,
                ..
            } => indices.get(index).map(|i| candidates[*i].id.as_str()),
        }
    }

    pub(super) fn enabled(&self, index: usize) -> bool {
        match &self.0 {
            Storage::Projected(rows) => rows.get(index).is_some_and(|row| row.enabled),
            Storage::Files {
                candidates,
                indices,
                ..
            } => indices
                .get(index)
                .is_some_and(|i| candidates[*i].availability.is_executable()),
        }
    }

    pub(super) fn position(&self, id: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.id(i) == Some(id))
    }

    // Menus and application icon requests only consult non-file rows.
    pub(in crate::launcher) fn projected(&self) -> &[LauncherRow] {
        match &self.0 {
            Storage::Projected(rows) => rows,
            Storage::Files { .. } => &[],
        }
    }
}

impl From<Vec<LauncherRow>> for LauncherRows {
    fn from(rows: Vec<LauncherRow>) -> Self {
        Self(Storage::Projected(rows))
    }
}

impl FromIterator<LauncherRow> for LauncherRows {
    fn from_iter<T: IntoIterator<Item = LauncherRow>>(rows: T) -> Self {
        Vec::from_iter(rows).into()
    }
}
