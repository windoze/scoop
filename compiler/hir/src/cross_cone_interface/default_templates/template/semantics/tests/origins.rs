use scoop_identity::{
    ConeIdentity, DefinitionOrigin, LocalValueSelector, NormalizedSourcePath, SourceContextKey,
    SourceIdentity, SourceSpan,
};

use super::*;
use crate::{
    DefaultTemplateOriginSemanticAuthority, ExportDefaultTemplateOriginSemanticValidationError,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceSemanticValidationError,
};

#[test]
fn validates_template_and_source_local_origins() {
    let fixture = Fixture::new();
    let template = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    let mut authority = fixture.authority();

    assert_eq!(template.validate_origin_semantics(&mut authority), Ok(()));
    assert_eq!(authority.definition_source_validations, 2);
    assert_eq!(authority.root_origin_validations, 1);
    assert_eq!(authority.local_origin_validations, 1);
}

#[test]
fn rejects_root_and_local_origins_from_another_cone() {
    let fixture = Fixture::new();
    let foreign = definition_source(ConeIdentity::CORE);
    let mut root = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    root.definition_origin = foreign.clone();

    assert_eq!(
        root.validate_origin_semantics(&mut fixture.authority()),
        Err(
            ExportDefaultTemplateOriginSemanticValidationError::DefinitionSource(
                ExportDefinitionSourceSemanticValidationError::Cone {
                    expected: ConeIdentity::SINGLE_FILE,
                    actual: ConeIdentity::CORE,
                }
            )
        )
    );

    let mut local = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    local.locals = parameter_locals(
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        CanonicalBooleanV1::False,
        foreign,
    );
    assert_eq!(
        local.validate_origin_semantics(&mut fixture.authority()),
        Err(
            ExportDefaultTemplateOriginSemanticValidationError::LocalSource {
                index: 0,
                selector: parameter_selector(0),
                error: ExportDefinitionSourceSemanticValidationError::Cone {
                    expected: ConeIdentity::SINGLE_FILE,
                    actual: ConeIdentity::CORE,
                },
            }
        )
    );
}

#[test]
fn reports_foundation_subject_relation_failures() {
    let fixture = Fixture::new();
    let template = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );

    let mut root_authority = fixture.authority();
    root_authority.reject_root_origin = true;
    assert_eq!(
        template.validate_origin_semantics(&mut root_authority),
        Err(
            ExportDefaultTemplateOriginSemanticValidationError::DefinitionRelation(
                AuthorityError::RootOrigin
            )
        )
    );

    let mut local_authority = fixture.authority();
    local_authority.reject_local_origin = true;
    assert_eq!(
        template.validate_origin_semantics(&mut local_authority),
        Err(
            ExportDefaultTemplateOriginSemanticValidationError::LocalRelation {
                index: 0,
                selector: parameter_selector(0),
                error: AuthorityError::LocalOrigin,
            }
        )
    );
}

#[test]
fn reports_common_foundation_source_failures_before_subject_relations() {
    let fixture = Fixture::new();
    let template = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    let mut authority = fixture.authority();
    authority.reject_definition_source = true;

    assert_eq!(
        template.validate_origin_semantics(&mut authority),
        Err(
            ExportDefaultTemplateOriginSemanticValidationError::DefinitionSource(
                ExportDefinitionSourceSemanticValidationError::Foundation(
                    AuthorityError::DefinitionSource
                )
            )
        )
    );
    assert_eq!(authority.root_origin_validations, 0);
}

impl ExportDefinitionSourceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::SINGLE_FILE
    }

    fn validate_export_definition_source(
        &mut self,
        _source: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        self.definition_source_validations += 1;
        if self.reject_definition_source {
            Err(AuthorityError::DefinitionSource)
        } else {
            Ok(())
        }
    }
}

impl DefaultTemplateOriginSemanticAuthority<AuthorityError> for Authority {
    fn validate_default_template_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        if key.owner() != self.declaration
            || root != self.provider
            || path != &self.definition_path
            || self.reject_root_origin
        {
            return Err(AuthorityError::RootOrigin);
        }
        self.root_origin_validations += 1;
        Ok(())
    }

    fn validate_default_template_local_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        selector: &LocalValueSelector,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        if key.owner() != self.declaration
            || root != self.provider
            || path != &self.definition_path
            || selector != &parameter_selector(0)
            || self.reject_local_origin
        {
            return Err(AuthorityError::LocalOrigin);
        }
        self.local_origin_validations += 1;
        Ok(())
    }
}

fn definition_source(cone: ConeIdentity) -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        cone,
        NormalizedSourcePath::new("src/provider.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(1, 2).unwrap(), &context).unwrap(),
    )
}
