use std::{collections::HashMap, fmt};

use scoop_source::{SourceEdit, source_span};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotEditError {
    Cancelled,
    MissingSource(SourceId),
    SourceChanged(SourceId),
    InvalidRange(SourceId),
    Overlap(SourceId),
    SourceLength(SourceLengthError),
}

impl fmt::Display for SnapshotEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("analysis revision changed"),
            Self::MissingSource(_) => formatter.write_str("edit source is unavailable"),
            Self::SourceChanged(_) => formatter.write_str("edit source version changed"),
            Self::InvalidRange(_) => {
                formatter.write_str("edit range is not a source character range")
            }
            Self::Overlap(_) => formatter.write_str("source edits overlap"),
            Self::SourceLength(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SnapshotEditError {}

impl AnalysisSnapshot {
    /// Create an unpublished candidate; the host and its cancellation are not changed.
    pub fn with_edits(&self, edits: &[SourceEdit]) -> Result<Self, SnapshotEditError> {
        self.check_cancelled()
            .map_err(|_| SnapshotEditError::Cancelled)?;
        if edits.is_empty() {
            return Ok(self.clone());
        }
        let mut grouped = HashMap::<_, Vec<_>>::new();
        for edit in edits {
            grouped.entry(edit.source.source).or_default().push(edit);
        }
        let revision = Revision(
            self.revision
                .0
                .checked_add(1)
                .expect("analysis revisions exhausted"),
        );
        let mut candidate = self.clone();
        candidate.revision = revision;
        for (id, mut edits) in grouped {
            self.check_cancelled()
                .map_err(|_| SnapshotEditError::Cancelled)?;
            let source = self
                .sources
                .get(&id)
                .ok_or(SnapshotEditError::MissingSource(id))?;
            edits.sort_by_key(|edit| (edit.span.start, edit.span.end));
            let mut length = source.text.as_str().len();
            for (index, edit) in edits.iter().enumerate() {
                if edit.source != source.version {
                    return Err(SnapshotEditError::SourceChanged(id));
                }
                if source
                    .text
                    .as_str()
                    .get(edit.span.start as usize..edit.span.end as usize)
                    .is_none()
                {
                    return Err(SnapshotEditError::InvalidRange(id));
                }
                if index > 0
                    && (edits[index - 1].span.end > edit.span.start
                        || edits[index - 1].span.start == edit.span.start)
                {
                    return Err(SnapshotEditError::Overlap(id));
                }
                length -= (edit.span.end - edit.span.start) as usize;
                length = length.checked_add(edit.replacement.len()).ok_or(
                    SnapshotEditError::SourceLength(SourceLengthError {
                        byte_length: usize::MAX,
                    }),
                )?;
            }
            source_span(length).map_err(SnapshotEditError::SourceLength)?;
            let mut text = source.text.as_str().to_owned();
            for edit in edits.into_iter().rev() {
                text.replace_range(
                    edit.span.start as usize..edit.span.end as usize,
                    &edit.replacement,
                );
            }
            if text == source.text.as_str() {
                continue;
            }
            Arc::make_mut(&mut candidate.sources).insert(
                id,
                Arc::new(SourceInput {
                    version: SourceVersion {
                        source: id,
                        revision,
                    },
                    text: SourceText::from(text),
                    parsed: OnceLock::new(),
                    symbols: OnceLock::new(),
                    documentation: OnceLock::new(),
                }),
            );
        }
        self.check_cancelled()
            .map_err(|_| SnapshotEditError::Cancelled)?;
        Ok(candidate)
    }
}
