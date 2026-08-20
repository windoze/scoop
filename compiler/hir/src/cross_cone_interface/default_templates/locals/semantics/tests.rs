use scoop_identity::{
    LocalValueSelector, PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    SyntheticLocalRole,
};

use super::*;
use crate::{
    CanonicalBooleanV1, PublicNominalShapeV1, SignatureBinderScopeError, TemplateLocalDefinitionV1,
    TemplateLocalRecordV1,
};

#[test]
fn accepts_locals_strictly_below_a_default_value_path() {
    let definition = path(&[(StructuralDefinitionSiteRole::DefaultValue, 1)]);
    let child = path(&[
        (StructuralDefinitionSiteRole::DefaultValue, 1),
        (StructuralDefinitionSiteRole::SyntheticValue, 0),
    ]);

    assert_eq!(table(child).validate_definition_path(&definition), Ok(()));
}

#[test]
fn rejects_non_default_definition_paths() {
    let definition = path(&[(StructuralDefinitionSiteRole::Lambda, 0)]);
    let child = path(&[
        (StructuralDefinitionSiteRole::Lambda, 0),
        (StructuralDefinitionSiteRole::SyntheticValue, 0),
    ]);

    assert_eq!(
        table(child).validate_definition_path(&definition),
        Err(
            TemplateLocalScopeValidationError::InvalidDefinitionPathRole {
                actual: StructuralDefinitionSiteRole::Lambda,
            }
        )
    );
}

#[test]
fn rejects_equal_and_sibling_local_paths() {
    let definition = path(&[(StructuralDefinitionSiteRole::DefaultValue, 1)]);
    let equal_selector = synthetic(definition.clone());
    assert_eq!(
        table(definition.clone()).validate_definition_path(&definition),
        Err(
            TemplateLocalScopeValidationError::LocalOutsideDefinitionPath {
                index: 0,
                selector: equal_selector,
            }
        )
    );

    let sibling = path(&[
        (StructuralDefinitionSiteRole::DefaultValue, 2),
        (StructuralDefinitionSiteRole::SyntheticValue, 0),
    ]);
    let sibling_selector = synthetic(sibling.clone());
    assert_eq!(
        table(sibling).validate_definition_path(&definition),
        Err(
            TemplateLocalScopeValidationError::LocalOutsideDefinitionPath {
                index: 0,
                selector: sibling_selector,
            }
        )
    );
}

#[test]
fn validates_local_types_in_the_provider_scope() {
    let child = path(&[
        (StructuralDefinitionSiteRole::DefaultValue, 1),
        (StructuralDefinitionSiteRole::SyntheticValue, 0),
    ]);
    let provider = DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap();
    let value_type = SignatureTypeKey::Binder { depth: 1, index: 0 };

    assert_eq!(
        table_with_type(child, value_type).validate_type_semantics(provider, &mut NoNominals),
        Ok(())
    );
}

#[test]
fn rejects_a_local_type_outside_the_provider_scope() {
    let child = path(&[
        (StructuralDefinitionSiteRole::DefaultValue, 1),
        (StructuralDefinitionSiteRole::SyntheticValue, 0),
    ]);
    let selector = synthetic(child.clone());
    let provider = DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap();

    assert_eq!(
        table_with_type(child, SignatureTypeKey::Binder { depth: 2, index: 0 },)
            .validate_type_semantics(provider, &mut NoNominals),
        Err(TemplateLocalTypeSemanticValidationError {
            index: 0,
            selector,
            error: SignatureTypeSemanticError::BinderScope(
                SignatureBinderScopeError::DepthOutOfRange {
                    depth: 2,
                    available_depths: 2,
                }
            ),
        })
    );
}

fn table(path: StructuralDefinitionPath) -> CanonicalTemplateLocalTableV1 {
    table_with_type(path, SignatureTypeKey::Binder { depth: 0, index: 0 })
}

fn table_with_type(
    path: StructuralDefinitionPath,
    value_type: SignatureTypeKey,
) -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            synthetic(path),
            value_type,
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Synthetic,
        )
        .unwrap(),
    ])
    .unwrap()
}

fn synthetic(path: StructuralDefinitionPath) -> LocalValueSelector {
    LocalValueSelector::Synthetic {
        path,
        role: SyntheticLocalRole::Temporary,
    }
}

fn path(segments: &[(StructuralDefinitionSiteRole, u32)]) -> StructuralDefinitionPath {
    let mut segments = segments
        .iter()
        .map(|(role, ordinal)| StructuralPathSegment::new(*role, *ordinal));
    let first = segments.next().unwrap();
    StructuralDefinitionPath::from_first(first, segments)
}

struct NoNominals;

impl NominalInterfaceShapeAuthority<std::convert::Infallible> for NoNominals {
    fn concrete_nominal_shape(
        &mut self,
        _declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, std::convert::Infallible> {
        unreachable!("binder-only test must not query nominal declarations")
    }

    fn generic_nominal_shape(
        &mut self,
        _declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, std::convert::Infallible> {
        unreachable!("binder-only test must not query nominal declarations")
    }
}
