use scoop_hir::{DefaultTemplateRootOriginValidationError as RootError, PersistentLexicalRootV1};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::default_fixture::{Case, fixture};
use super::*;
use crate::CrossConeHirDefaultRootOriginError;

mod support;
use support::{add_sibling, replace_root};

#[test]
fn the_ordinary_reader_rejects_a_different_root_with_the_same_signature() {
    let mut fixture = fixture(Case::Defined);
    let (other, _) = add_sibling(&mut fixture, false);
    let original = fixture.interface.default_templates().records()[0].clone();
    replace_root(&mut fixture, other, original.definition_origin().clone());
    assert!(matches!(failure(&fixture), RootError::RootOrigin(root) if root == other));
}

#[test]
fn the_ordinary_reader_rejects_another_callables_context_in_the_same_file() {
    let mut fixture = fixture(Case::Defined);
    let (_, origin) = add_sibling(&mut fixture, false);
    let root = PersistentLexicalRootV1::try_from(fixture.owner).unwrap();
    replace_root(&mut fixture, root, origin);
    assert!(matches!(failure(&fixture), RootError::RootOrigin(actual) if actual == root));
}

#[test]
fn the_ordinary_reader_rejects_a_valid_context_from_another_file() {
    let mut fixture = fixture(Case::Defined);
    let (_, origin) = add_sibling(&mut fixture, true);
    let root = PersistentLexicalRootV1::try_from(fixture.owner).unwrap();
    replace_root(&mut fixture, root, origin);
    assert!(matches!(failure(&fixture), RootError::RootOrigin(actual) if actual == root));
}

#[test]
fn the_ordinary_reader_rejects_a_file_context_for_a_callable_default() {
    let mut fixture = fixture(Case::Defined);
    let foundation = scoop_hir::OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap();
    let CallableTemplateOrigin::Function(id) = fixture.owner else {
        panic!("fixture function")
    };
    let origin = foundation
        .definition_origin(DefinitionOriginSubject::Function(id))
        .unwrap();
    assert!(matches!(
        foundation.source_context_key(origin.origin().context()),
        Some(SourceContextKey::File { .. })
    ));
    replace_root(
        &mut fixture,
        PersistentLexicalRootV1::Function(id),
        ExportDefinitionSourceV1::new(origin.origin().clone()),
    );
    assert!(matches!(failure(&fixture), RootError::RootOrigin(_)));
}

#[test]
fn a_default_root_requires_artifact_membership_and_the_same_identity_graph() {
    let fixture = fixture(Case::Defined);
    let bytes = fixture.artifact();
    let front = validate_until_type_alias(&bytes);
    let provider = front.nominal_provider_view();
    let template = &front.hir_interface().default_templates().records()[0];
    let mut missing = provider.foundation.as_canonical().clone();
    missing.set_functions(vec![]).unwrap();
    let missing = scoop_hir::OdrFreeHirFoundation::try_new(missing).unwrap();
    let empty_graph = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    for (foundation, identities, identity, expected) in [
        (
            &missing,
            provider.identities,
            provider.identity,
            "absent from its provider artifact",
        ),
        (
            provider.foundation,
            &empty_graph,
            provider.identity,
            "identity validation failed",
        ),
        (
            provider.foundation,
            provider.identities,
            ConeIdentity::SINGLE_FILE,
            "another provider",
        ),
    ] {
        let error = foundation
            .validate_default_template_root_origin(
                identity,
                identities,
                template.definition_root(),
                template.definition_origin(),
                &mut BudgetMeter::new(DecodeLimits::default()),
                &WirePath::root(),
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn shared_default_root_validation_spends_the_remaining_budget() {
    let fixture = fixture(Case::Defined);
    let bytes = fixture.artifact();
    let front = validate_until_type_alias(&bytes);
    let provider = front.nominal_provider_view();
    let template = &front.hir_interface().default_templates().records()[0];
    let validate = |meter: &mut BudgetMeter| {
        provider.foundation.validate_default_template_root_origin(
            provider.identity,
            provider.identities,
            template.definition_root(),
            template.definition_origin(),
            meter,
            &WirePath::root(),
        )
    };
    for limits in [
        DecodeLimits {
            semantic_recursion: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_leaf_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            validate(&mut BudgetMeter::new(limits)),
            Err(RootError::Resource(_))
        ));
    }
    let mut measured = BudgetMeter::new(DecodeLimits::default());
    validate(&mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units * 2 - 1,
        ..DecodeLimits::default()
    });
    validate(&mut shared).unwrap();
    assert!(matches!(validate(&mut shared), Err(RootError::Resource(_))));
}

fn failure(fixture: &CallableSourceSurface) -> RootError {
    let bytes = fixture.artifact();
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultRootOrigin(
        CrossConeHirDefaultRootOriginError::Template { index, key, source },
    )) = validate_until_type_alias(&bytes).validate_source_interfaces(vec![])
    else {
        panic!("a mismatched default root must not pass the source-interface gate");
    };
    assert_eq!(index, 0);
    assert_eq!(key.owner(), fixture.owner);
    assert_eq!(key.parameter_position(), 0);
    assert!(
        source
            .to_string()
            .contains("no matching declaration source/context")
    );
    *source
}
