use scoop_identity::{
    ConeIdentity, DefinitionOrigin, NormalizedSourcePath, SourceContextKey, SourceIdentity,
    SourceSpan,
};

use super::*;
use crate::{CanonicalExportDefinitionSourcesV1, ExportDefinitionSourceSetSemanticValidationError};

#[test]
fn validates_current_cone_source_context_and_points() {
    let source = definition_source(ConeIdentity::CORE);
    let mut authority = Authority {
        cone: ConeIdentity::CORE,
        reject: false,
        validations: 0,
    };

    assert_eq!(source.validate_semantics(&mut authority), Ok(()));
    assert_eq!(authority.validations, 1);
}

#[test]
fn rejects_a_source_from_another_cone_before_foundation_lookup() {
    let source = definition_source(ConeIdentity::CORE);
    let mut authority = Authority {
        cone: ConeIdentity::SINGLE_FILE,
        reject: false,
        validations: 0,
    };

    assert_eq!(
        source.validate_semantics(&mut authority),
        Err(ExportDefinitionSourceSemanticValidationError::Cone {
            expected: ConeIdentity::SINGLE_FILE,
            actual: ConeIdentity::CORE,
        })
    );
    assert_eq!(authority.validations, 0);
}

#[test]
fn reports_foundation_source_validation_failure() {
    let source = definition_source(ConeIdentity::CORE);
    let mut authority = Authority {
        cone: ConeIdentity::CORE,
        reject: true,
        validations: 0,
    };

    assert_eq!(
        source.validate_semantics(&mut authority),
        Err(ExportDefinitionSourceSemanticValidationError::Foundation(
            SourceError
        ))
    );
    assert_eq!(authority.validations, 1);
}

#[test]
fn table_validates_every_source_in_canonical_order() {
    let sources = CanonicalExportDefinitionSourcesV1::try_new(vec![
        definition_source_with_span(ConeIdentity::CORE, 3, 4),
        definition_source_with_span(ConeIdentity::CORE, 1, 2),
    ])
    .unwrap();
    let mut authority = Authority {
        cone: ConeIdentity::CORE,
        reject: false,
        validations: 0,
    };

    assert_eq!(sources.validate_semantics(&mut authority), Ok(()));
    assert_eq!(authority.validations, 2);
}

#[test]
fn table_reports_the_failing_source_index() {
    let sources =
        CanonicalExportDefinitionSourcesV1::try_new(vec![definition_source(ConeIdentity::CORE)])
            .unwrap();
    let mut authority = Authority {
        cone: ConeIdentity::CORE,
        reject: true,
        validations: 0,
    };

    assert_eq!(
        sources.validate_semantics(&mut authority),
        Err(ExportDefinitionSourceSetSemanticValidationError::Source {
            index: 0,
            error: ExportDefinitionSourceSemanticValidationError::Foundation(SourceError),
        })
    );
    assert_eq!(authority.validations, 1);
}

struct Authority {
    cone: ConeIdentity,
    reject: bool,
    validations: usize,
}

impl ExportDefinitionSourceSemanticAuthority<SourceError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.cone
    }

    fn validate_export_definition_source(
        &mut self,
        _source: &ExportDefinitionSourceV1,
    ) -> Result<(), SourceError> {
        self.validations += 1;
        if self.reject {
            Err(SourceError)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceError;

impl std::fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid source")
    }
}

impl std::error::Error for SourceError {}

fn definition_source(cone: ConeIdentity) -> ExportDefinitionSourceV1 {
    definition_source_with_span(cone, 1, 2)
}

fn definition_source_with_span(
    cone: ConeIdentity,
    start: u64,
    end: u64,
) -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        cone,
        NormalizedSourcePath::new("src/default.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(start, end).unwrap(), &context).unwrap(),
    )
}
