use super::super::surface_fixture::declaration_front;
use super::*;
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirDefaultNestedIdentityError as Error,
};
use scoop_hir::{
    DefaultNestedIdentityValidationError as IdentityError,
    DefaultSourceNestedIdentityFailureV1 as Failure, *,
};
use scoop_identity::{
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

#[test]
fn ordinary_reader_requires_a_local_function_references_body_declaration() {
    let mut fixture = fixture(0);
    let template = &fixture.interface.default_templates().records()[0];
    let statements = template
        .body()
        .statements()
        .iter()
        .filter(|statement| !matches!(statement.kind(), DefaultStatementKindV1::LocalFunction(_)))
        .cloned()
        .collect();
    let locals = template.locals().records().to_vec();
    default_envelope::support::replace_contents(&mut fixture, locals, statements);
    let bytes = fixture.artifact();
    let mut front = declaration_front(&bytes);
    let Err(Error::Template { source, .. }) = validate(&mut front) else {
        panic!("a local function reference requires its body declaration")
    };
    assert!(matches!(
        *source,
        Error::Reference(DefaultSourceNestedCallableQueryError::MissingIdentity(_))
    ));
}

#[test]
fn ordinary_reader_checks_a_nested_descriptors_actual_owner_binder_count() {
    let fixture = fixture(4);
    assert!(matches!(
        failure(&fixture),
        IdentityError::Identity {
            reason: Failure::OwnerBinderArity {
                expected: 0,
                actual: 4
            },
            ..
        }
    ));
}

#[test]
fn ordinary_reader_matches_local_descriptor_paths_to_artifact_keys() {
    let mut fixture = fixture(0);
    let template = &fixture.interface.default_templates().records()[0];
    let statements = template
        .body()
        .statements()
        .iter()
        .map(|statement| {
            let DefaultStatementKindV1::LocalFunction(local) = statement.kind() else {
                return statement.clone();
            };
            let path = StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
                [StructuralPathSegment::new(
                    StructuralDefinitionSiteRole::LocalDeclaration,
                    2,
                )],
            );
            let changed = DefaultLocalFunctionV1::try_new(
                local.declaration(),
                path,
                local.function_type().clone(),
                local.captures().to_vec(),
                local.owner_type_parameter_count(),
            )
            .unwrap();
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::LocalFunction(changed),
                statement.definition_origin().clone(),
            )
            .unwrap()
        })
        .collect();
    let locals = template.locals().records().to_vec();
    default_envelope::support::replace_contents(&mut fixture, locals, statements);
    assert!(matches!(
        failure(&fixture),
        IdentityError::Identity {
            reason: Failure::DefinitionPath,
            ..
        }
    ));
}

#[test]
fn ordinary_nested_identities_require_the_actual_artifact_record_in_addition_to_the_graph() {
    let fixture = fixture(0);
    let bytes = fixture.artifact();
    let mut front = declaration_front(&bytes);
    validate(&mut front).unwrap();
    let mut foundation = front.foundations.hir.as_canonical().clone();
    foundation.set_generic_functions(vec![]).unwrap();
    front.foundations.hir = OdrFreeHirFoundation::try_new(foundation).unwrap();
    let Err(Error::Template { source, .. }) = validate(&mut front) else {
        panic!("missing artifact key must fail")
    };
    let Error::Occurrence { source, .. } = *source else {
        panic!("nested occurrence error")
    };
    assert!(matches!(
        *source,
        IdentityError::Identity {
            reason: Failure::MissingArtifactRecord,
            ..
        }
    ));
}

fn fixture(owner_binders: u32) -> CallableSourceSurface {
    default_envelope::local_function_fixture(
        1,
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        owner_binders,
    )
}

fn failure(fixture: &CallableSourceSurface) -> IdentityError {
    let bytes = fixture.artifact();
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultNestedIdentity(Error::Template {
        index,
        key,
        source,
    })) = validate_until_type_alias(&bytes).validate_source_interfaces(vec![])
    else {
        panic!("invalid nested identity must fail ordinary source-interface validation")
    };
    assert_eq!(index, 0);
    assert_eq!(key.owner(), fixture.owner);
    let Error::Occurrence { site, source } = *source else {
        panic!("nested occurrence error")
    };
    assert_eq!(site, DefaultNestedCallableSiteV1::Body { ordinal: 0 });
    *source
}

fn validate(front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>) -> Result<(), Error> {
    CanonicalCrossConeHirSurfaceAuthority::new(
        front.graph.identity(),
        &front.identities,
        &front.foundations.hir,
        &front.hir_interface,
        vec![],
    )
    .validate_default_nested_identities()
}
