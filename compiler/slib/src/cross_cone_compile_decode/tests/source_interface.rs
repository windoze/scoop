use scoop_hir::{
    CallableParameterCallingV1, CallableSourceInterfaceSemanticValidationError,
    CallableSourceInterfaceSetSemanticValidationError, CallableSourceInterfaceV1,
    CallableSourceParameterV1, CanonicalBinderListV1, CanonicalCallableInterfacesV1,
    CanonicalCallableSourceInterfacesV1, CanonicalCallableSourceParametersV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1, CanonicalPropertyInterfacesV1,
    CanonicalPublicExportBindingsV1, CanonicalPublicMemberRefsV1, CanonicalSignatureTypesV1,
    CanonicalSourceParameterShapesV1, CanonicalTypeAliasInterfacesV1,
    CrossConeHirInterfaceSectionV1, ExportDefinitionSourceV1, NominalSourceShapeV1,
    PublicDeclarationOwnerV1, PublicLookupAccessV1, PublicNominalKindV1, SourceNominalId,
    SourceParameterShapeV1, StructSourceShapeV1,
};
use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, DeclarationScope,
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerChain,
    NormalizedSourcePath, PackagePath, PersistentFunctionId, PersistentSourceContextId,
    PersistentTypeId, SignatureTypeKey, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::encode;

use super::*;
use crate::cross_cone_hir_authority::CrossConeHirCallableSourceAuthorityError;

mod default_data_flow;

#[test]
fn vararg_reader_follows_the_actual_intrinsic_array_on_an_ordinary_provider() {
    let fixture = CallableSourceSurface::new(SourceInterfaceCase::IntrinsicArrayVararg);
    let bytes = fixture.artifact();
    let validated = validate_until_type_alias(&bytes)
        .validate_source_interfaces(vec![])
        .unwrap();
    assert_ne!(validated.identity(), ConeIdentity::CORE);
    assert!(
        validated
            .hir_core_production()
            .compiler_protocol_definitions()
            .is_none()
    );
}

#[test]
fn validates_a_complete_callable_source_interface() {
    let fixture = CallableSourceSurface::new(SourceInterfaceCase::Complete);
    let bytes = fixture.artifact();
    let validated = validate_until_type_alias(&bytes)
        .validate_source_interfaces(Vec::new())
        .unwrap();

    assert_eq!(validated.identity(), fixture.cone.identity());
    assert!(
        validated
            .hir_interface()
            .source_interfaces()
            .get(fixture.owner)
            .is_some()
    );
}

#[test]
fn rejects_a_callable_without_a_source_interface() {
    let fixture = CallableSourceSurface::new(SourceInterfaceCase::Missing);
    let bytes = fixture.artifact();
    let front = validate_until_type_alias(&bytes);

    let Err(CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces(error)) =
        front.validate_source_interfaces(Vec::new())
    else {
        panic!("a public callable without a source interface must fail validation");
    };
    assert!(matches!(
        error,
        CallableSourceInterfaceSetSemanticValidationError::MissingSourceInterface(owner)
            if owner == fixture.owner
    ));
}

#[test]
fn rejects_a_parameter_origin_from_a_different_source() {
    let fixture = CallableSourceSurface::new(SourceInterfaceCase::DifferentParameterSource);
    let bytes = fixture.artifact();
    let front = validate_until_type_alias(&bytes);

    let Err(CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces(error)) =
        front.validate_source_interfaces(Vec::new())
    else {
        panic!("a parameter from another source must fail source-interface validation");
    };
    assert!(matches!(
        error,
        CallableSourceInterfaceSetSemanticValidationError::Record {
            error: CallableSourceInterfaceSemanticValidationError::DefinitionOrigin {
                index: 0,
                error: CrossConeHirCallableSourceAuthorityError::ParameterSourceMismatch {
                    owner,
                    position: 0,
                },
            },
            ..
        } if owner == fixture.owner
    ));
}

#[test]
fn rejects_a_vararg_with_a_non_array_parameter_type() {
    let fixture = CallableSourceSurface::new(SourceInterfaceCase::NonArrayVararg);
    let bytes = fixture.artifact();
    let front = validate_until_type_alias(&bytes);

    let Err(CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces(error)) =
        front.validate_source_interfaces(Vec::new())
    else {
        panic!("a vararg with a non-array parameter type must fail validation");
    };
    assert!(matches!(
        error,
        CallableSourceInterfaceSetSemanticValidationError::Record {
            error: CallableSourceInterfaceSemanticValidationError::VarargArrayType { index: 0, .. },
            ..
        }
    ));
}

fn validate_until_type_alias(bytes: &[u8]) -> TypeAliasValidatedCrossConeHirFrontSections<'_> {
    let mut decoded = open_graph(bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources()
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(Vec::new())
        .unwrap()
        .validate_type_alias_surface(Vec::new())
        .unwrap()
}

#[derive(Clone, Copy)]
enum SourceInterfaceCase {
    Complete,
    Missing,
    DifferentParameterSource,
    NonArrayVararg,
    IntrinsicArrayVararg,
}

struct CallableSourceSurface {
    cone: ConeRecord,
    foundation: CanonicalHirFoundation,
    interface: CrossConeHirInterfaceSectionV1,
    owner: CallableTemplateOrigin,
}

impl CallableSourceSurface {
    fn new(case: SourceInterfaceCase) -> Self {
        let cone = cone();
        let identity = cone.identity();
        let declaration_source = SourceIdentity::new(
            identity,
            NormalizedSourcePath::new("src/Callable.scoop").unwrap(),
        )
        .unwrap();
        let declaration_context_key = SourceContextKey::File {
            source: declaration_source.clone(),
        };
        let declaration_context = CborIdentityRecord::<PersistentSourceContextId, _>::from_key(
            declaration_context_key.clone(),
        )
        .unwrap();
        let nominal_origin = DefinitionOrigin::new(
            declaration_source.clone(),
            SourceSpan::new(0, 5).unwrap(),
            &declaration_context_key,
        )
        .unwrap();
        let callable_origin = DefinitionOrigin::new(
            declaration_source.clone(),
            SourceSpan::new(6, 13).unwrap(),
            &declaration_context_key,
        )
        .unwrap();
        let declaration_site = SourceDeclarationSite::new(
            identity,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let nominal =
            CborIdentityRecord::<PersistentTypeId, _>::from_key(SourceDeclarationKey::nominal(
                declaration_site.clone(),
                CanonicalIdentifier::new("Value").unwrap(),
                SourceNominalKind::Struct,
                0,
            ))
            .unwrap();
        let array = CborIdentityRecord::<scoop_identity::PersistentGenericTypeId, _>::from_key(
            SourceDeclarationKey::nominal(
                declaration_site.clone(),
                CanonicalIdentifier::new("ValueArray").unwrap(),
                SourceNominalKind::Class,
                1,
            ),
        )
        .unwrap();
        let element_type = SignatureTypeKey::Nominal(nominal.id());
        let value_type = if matches!(case, SourceInterfaceCase::IntrinsicArrayVararg) {
            SignatureTypeKey::NominalApplication {
                origin: array.id(),
                arguments: scoop_identity::NonEmptyVec::from_first(element_type.clone(), []),
            }
        } else {
            element_type.clone()
        };
        let function = CborIdentityRecord::<PersistentFunctionId, _>::from_key(
            SourceDeclarationKey::function(
                declaration_site,
                CanonicalIdentifier::new("consume").unwrap(),
                0,
                None,
                vec![value_type.clone()],
            ),
        )
        .unwrap();
        let owner = CallableTemplateOrigin::Function(function.id());

        let (parameter_origin, mut source_records, mut contexts) = match case {
            SourceInterfaceCase::DifferentParameterSource => {
                let source = SourceIdentity::new(
                    identity,
                    NormalizedSourcePath::new("src/Other.scoop").unwrap(),
                )
                .unwrap();
                let context_key = SourceContextKey::File {
                    source: source.clone(),
                };
                let context = CborIdentityRecord::<PersistentSourceContextId, _>::from_key(
                    context_key.clone(),
                )
                .unwrap();
                let origin = DefinitionOrigin::new(
                    source.clone(),
                    SourceSpan::new(0, 5).unwrap(),
                    &context_key,
                )
                .unwrap();
                (
                    origin,
                    vec![scoop_hir::SourceRecord::from_utf8(source, "other", [0, 5]).unwrap()],
                    vec![context],
                )
            }
            SourceInterfaceCase::Complete
            | SourceInterfaceCase::Missing
            | SourceInterfaceCase::NonArrayVararg
            | SourceInterfaceCase::IntrinsicArrayVararg => (
                DefinitionOrigin::new(
                    declaration_source.clone(),
                    SourceSpan::new(14, 19).unwrap(),
                    &declaration_context_key,
                )
                .unwrap(),
                Vec::new(),
                Vec::new(),
            ),
        };
        source_records.push(
            scoop_hir::SourceRecord::from_utf8(
                declaration_source,
                "Value consume(value)",
                [0, 5, 6, 13, 14, 19],
            )
            .unwrap(),
        );
        contexts.push(declaration_context);

        let mut foundation = base_hir_foundation();
        foundation.set_sources(source_records).unwrap();
        foundation.set_source_contexts(contexts).unwrap();
        foundation
            .set_definition_origins(vec![
                DefinitionOriginRecord::new(
                    DefinitionOriginSubject::GenericType(array.id()),
                    nominal_origin.clone(),
                ),
                DefinitionOriginRecord::new(
                    DefinitionOriginSubject::Type(nominal.id()),
                    nominal_origin,
                ),
                DefinitionOriginRecord::new(
                    DefinitionOriginSubject::Function(function.id()),
                    callable_origin,
                ),
            ])
            .unwrap();
        foundation
            .set_types(vec![
                scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
                scoop_identity::CoreBuiltinNominal::Any.identity_record(),
                nominal.clone(),
            ])
            .unwrap();
        foundation.set_functions(vec![function]).unwrap();
        foundation.set_generic_types(vec![array.clone()]).unwrap();

        let nominal_interface = crate::nominal_interface_fixture::public_record(
            SourceNominalId::Concrete(nominal.id()),
            PublicNominalKindV1::Struct,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            NominalSourceShapeV1::Struct(
                StructSourceShapeV1::try_new(
                    Vec::new(),
                    scoop_hir::NominalCLayoutPolicyV1::Ordinary,
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let array_interface = crate::nominal_interface_fixture::public_record(
            SourceNominalId::GenericTemplate(array.id()),
            PublicNominalKindV1::Class,
            CanonicalBinderListV1::try_new(vec![scoop_hir::TypeParameterBinderV1::new(
                CanonicalIdentifier::new("T").unwrap(),
                scoop_hir::TypeParameterBoundsV1::Unconstrained,
            )])
            .unwrap(),
            CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
            NominalSourceShapeV1::Intrinsic(scoop_hir::NominalIntrinsicRepresentationV1::new(
                scoop_hir::IntrinsicTypeKind::Array,
            )),
        )
        .unwrap();
        let callable_interface = CallableInterfaceRecordV1::try_new(
            owner,
            PublicDeclarationOwnerV1::TopLevel,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
                CanonicalIdentifier::new("value").unwrap(),
                value_type.clone(),
            )])
            .unwrap(),
            value_type.clone(),
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
            scoop_hir::CanonicalPersistentIdsV1::empty(),
        )
        .unwrap();
        let calling = if matches!(
            case,
            SourceInterfaceCase::NonArrayVararg | SourceInterfaceCase::IntrinsicArrayVararg
        ) {
            CallableParameterCallingV1::VarargEmpty { element_type }
        } else {
            CallableParameterCallingV1::Required
        };
        let source_interfaces = if matches!(case, SourceInterfaceCase::Missing) {
            Vec::new()
        } else {
            vec![
                CallableSourceInterfaceV1::try_new(
                    owner,
                    CanonicalCallableSourceParametersV1::try_new(vec![
                        CallableSourceParameterV1::new(
                            CanonicalIdentifier::new("value").unwrap(),
                            value_type,
                            calling,
                            ExportDefinitionSourceV1::new(parameter_origin.clone()),
                        ),
                    ])
                    .unwrap(),
                )
                .unwrap(),
            ]
        };
        let definition_sources = if matches!(case, SourceInterfaceCase::Missing) {
            Vec::new()
        } else {
            vec![ExportDefinitionSourceV1::new(parameter_origin)]
        };
        let section = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
            CanonicalNominalInterfacesV1::try_new(vec![nominal_interface, array_interface])
                .unwrap(),
            CanonicalCallableInterfacesV1::try_new(vec![callable_interface]).unwrap(),
            CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalCallableSourceInterfacesV1::try_new(source_interfaces).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(definition_sources).unwrap(),
            CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
        );

        Self {
            cone,
            foundation,
            interface: section,
            owner,
        }
    }

    fn artifact(&self) -> Vec<u8> {
        let mut interface = self.interface.clone();
        cross_cone_artifact_for_with_hir_foundation(
            self.cone.clone(),
            Vec::new(),
            &self.foundation,
            encode(&interface.index_for_wire().unwrap()).unwrap(),
        )
    }
}
