//! Canonical definition origins established by Export HIR.

use std::fmt;

use scoop_identity::{DefinitionOriginRecord, DefinitionOriginSubject};

/// Canonical definition origins for every foundation subject whose identity
/// is established by Export HIR. LocalConcrete local values are intentionally
/// outside this relation because their identities do not exist until exact
/// callable materialization.
#[derive(Clone, Debug, Default)]
pub struct HirExportDefinitionOrigins {
    records: Vec<DefinitionOriginRecord>,
}

impl HirExportDefinitionOrigins {
    pub fn canonicalize(
        mut records: Vec<DefinitionOriginRecord>,
    ) -> Result<Self, HirExportDefinitionOriginError> {
        records.sort_by(|left, right| left.subject().compare_sort_key(right.subject()));
        if let Some(pair) = records.windows(2).find(|pair| {
            pair[0]
                .subject()
                .compare_sort_key(pair[1].subject())
                .is_eq()
        }) {
            return Err(HirExportDefinitionOriginError::DuplicateSubject(
                pair[0].subject(),
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[DefinitionOriginRecord] {
        &self.records
    }

    pub fn get(&self, subject: DefinitionOriginSubject) -> Option<&DefinitionOriginRecord> {
        self.records
            .binary_search_by(|record| record.subject().compare_sort_key(subject))
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirExportDefinitionOriginError {
    DuplicateSubject(DefinitionOriginSubject),
}

impl fmt::Display for HirExportDefinitionOriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSubject(subject) => write!(
                formatter,
                "definition-origin subject {}:{:?} appears more than once",
                subject.kind_tag(),
                subject.raw_id()
            ),
        }
    }
}

impl std::error::Error for HirExportDefinitionOriginError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CoreBuiltinNominal, DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject,
        SourceContextKey, SourceIdentity, SourceSpan,
    };

    use super::*;

    fn record(subject: DefinitionOriginSubject) -> DefinitionOriginRecord {
        let source = SourceIdentity::single_file();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let origin =
            DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap();
        DefinitionOriginRecord::new(subject, origin)
    }

    #[test]
    fn canonicalization_rejects_duplicate_subjects() {
        let subject =
            DefinitionOriginSubject::Type(CoreBuiltinNominal::Unit.identity_record().id());
        assert_eq!(
            HirExportDefinitionOrigins::canonicalize(vec![record(subject), record(subject)])
                .unwrap_err(),
            HirExportDefinitionOriginError::DuplicateSubject(subject)
        );
    }
}
