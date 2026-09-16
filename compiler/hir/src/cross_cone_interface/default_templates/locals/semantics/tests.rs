use scoop_identity::{
    LocalValueSelector, SignatureTypeKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment, SyntheticLocalRole,
};

use super::*;
use crate::{CanonicalBooleanV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1};

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

fn table(path: StructuralDefinitionPath) -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            synthetic(path),
            SignatureTypeKey::Binder { depth: 0, index: 0 },
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
