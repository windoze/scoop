use scoop_hir::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use scoop_hir::{
    CallableInterfaceRecordV1, CallableModalityV1, CanonicalBinderListV1, CanonicalBooleanV1,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1, CanonicalConstValueV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1, CanonicalPropertyInterfacesV1,
    CanonicalPublicExportBindingsV1, CanonicalPublicMemberRefsV1, CanonicalSignatureTypesV1,
    CanonicalSourceParameterShapesV1, CanonicalTypeAliasInterfacesV1,
    CrossConeHirInterfaceSectionV1, ExportConstValueSemanticValidationError,
    ExportConstValueSetSemanticValidationError, ExportConstValueV1, ExportDefinitionSourceV1,
    NominalInterfaceRecordV1, NominalSourceShapeV1, PropertyInterfaceRecordV1,
    PropertyPublicAccessV1, PropertyRepresentationV1, PublicDeclarationOwnerV1,
    PublicLookupAccessV1, PublicNominalKindV1, SourceNominalId, StructSourceShapeV1,
};
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    DeclarationScope, DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject,
    DefinitionOwnerChain, NormalizedSourcePath, PackagePath, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentSourceContextId, PersistentTypeId, PropertyAccessorKey,
    PropertyOwner, SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::encode;

use super::*;
use crate::cross_cone_hir_authority::{
    CrossConeHirConstAuthorityError, CrossConeHirIntrinsicTypeError,
};

mod intrinsic_providers;

#[test]
fn const_reader_uses_an_ordinary_providers_explicit_intrinsic_declaration() {
    let fixture = ConstSurface::new(ConstSurfaceCase::IntrinsicBoolean);
    let bytes = fixture.artifact();
    let validated = validate_until_source_interfaces(&bytes)
        .validate_const_values(vec![])
        .unwrap();
    assert_ne!(validated.identity(), ConeIdentity::CORE);
    assert!(
        validated
            .hir_core_production()
            .compiler_protocols()
            .is_none()
    );
    assert_eq!(validated.hir_interface().constants().records().len(), 1);
}

#[test]
fn const_reader_rejects_the_wrong_intrinsic_family_at_the_actual_type() {
    let fixture = ConstSurface::new(ConstSurfaceCase::WrongFamily);
    let bytes = fixture.artifact();
    let front = validate_until_source_interfaces(&bytes);
    let Err(CrossConeHirConstSurfaceError::Constants(error)) = front.validate_const_values(vec![])
    else {
        panic!("a Boolean constant cannot use an integer intrinsic declaration");
    };
    assert!(matches!(
        error,
        ExportConstValueSetSemanticValidationError::Record {
            error: ExportConstValueSemanticValidationError::ValueType {
                error: CrossConeHirConstAuthorityError::ValueType(
                    CrossConeHirIntrinsicTypeError::Family {
                        expected: scoop_hir::IntrinsicTypeKind::Boolean,
                        actual: scoop_hir::IntrinsicTypeKind::Integer(
                            scoop_hir::IntegerKind::SIGNED_32
                        ),
                        ..
                    }
                ),
                ..
            },
            ..
        }
    ));
}

#[test]
fn validates_an_empty_const_surface() {
    let bytes = cross_cone_artifact(empty_cross_cone_hir_interface());
    let validated = validate_until_source_interfaces(&bytes)
        .validate_const_values(Vec::new())
        .unwrap();

    assert_eq!(validated.identity(), cone().identity());
    assert!(validated.hir_interface().constants().is_empty());
}

#[test]
fn rejects_an_ordinary_nominal_as_a_const_intrinsic_type() {
    let fixture = ConstSurface::new(ConstSurfaceCase::OrdinaryNominal);
    let bytes = fixture.artifact();
    let front = validate_until_source_interfaces(&bytes);

    let Err(CrossConeHirConstSurfaceError::Constants(error)) =
        front.validate_const_values(Vec::new())
    else {
        panic!("an ordinary nominal cannot supply the const intrinsic family");
    };
    assert!(matches!(
        error,
        ExportConstValueSetSemanticValidationError::Record {
            error: ExportConstValueSemanticValidationError::ValueType {
                error: CrossConeHirConstAuthorityError::ValueType(
                    CrossConeHirIntrinsicTypeError::NotIntrinsic { .. }
                ),
                ..
            },
            ..
        }
    ));
}

#[test]
fn rejects_a_const_origin_that_differs_from_its_property() {
    let fixture = ConstSurface::new(ConstSurfaceCase::MismatchedOrigin);
    let bytes = fixture.artifact();
    let front = validate_until_source_interfaces(&bytes);

    let Err(CrossConeHirConstSurfaceError::Constants(error)) =
        front.validate_const_values(Vec::new())
    else {
        panic!("a mismatched const origin must fail validation");
    };
    assert!(matches!(
        error,
        ExportConstValueSetSemanticValidationError::Record {
            error: ExportConstValueSemanticValidationError::DefinitionOriginMismatch { .. },
            ..
        }
    ));
}

fn validate_until_source_interfaces(
    bytes: &[u8],
) -> SourceInterfaceValidatedCrossConeHirFrontSections<'_> {
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
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(Vec::new())
        .unwrap()
        .validate_type_alias_surface(Vec::new())
        .unwrap()
        .validate_source_interfaces(Vec::new())
        .unwrap()
}

#[derive(Clone, Copy)]
enum ConstSurfaceCase {
    OrdinaryNominal,
    MismatchedOrigin,
    IntrinsicBoolean,
    WrongFamily,
}

struct ConstSurface {
    cone: ConeRecord,
    foundation: CanonicalHirFoundation,
    interface: Vec<u8>,
}

impl ConstSurface {
    fn new(case: ConstSurfaceCase) -> Self {
        let cone = cone();
        let identity = cone.identity();
        let source = SourceIdentity::new(
            identity,
            NormalizedSourcePath::new("src/Constants.scoop").unwrap(),
        )
        .unwrap();
        let context_key = SourceContextKey::File {
            source: source.clone(),
        };
        let context =
            CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context_key.clone())
                .unwrap();
        let type_origin =
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 5).unwrap(), &context_key)
                .unwrap();
        let property_origin = DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(6, 12).unwrap(),
            &context_key,
        )
        .unwrap();
        let mismatched_origin = DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(13, 18).unwrap(),
            &context_key,
        )
        .unwrap();
        let interface_origin = if matches!(case, ConstSurfaceCase::MismatchedOrigin) {
            mismatched_origin
        } else {
            property_origin.clone()
        };
        let site = SourceDeclarationSite::new(
            identity,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let value_type =
            CborIdentityRecord::<PersistentTypeId, _>::from_key(SourceDeclarationKey::nominal(
                site.clone(),
                CanonicalIdentifier::new("BooleanLike").unwrap(),
                SourceNominalKind::Struct,
                0,
            ))
            .unwrap();
        let property = CborIdentityRecord::<PersistentPropertyId, _>::from_key(
            SourceDeclarationKey::property(site, CanonicalIdentifier::new("answer").unwrap()),
        )
        .unwrap();
        let getter = CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(
            PropertyAccessorKey::new(PropertyOwner::Property(property.id()), AccessorRole::Getter),
        )
        .unwrap();

        let mut foundation = base_hir_foundation();
        foundation
            .set_sources(vec![
                scoop_hir::SourceRecord::from_utf8(
                    source,
                    "Value answer extra",
                    [0, 5, 6, 12, 13, 18],
                )
                .unwrap(),
            ])
            .unwrap();
        foundation.set_source_contexts(vec![context]).unwrap();
        let origins = vec![
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Type(value_type.id()),
                type_origin,
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::PropertyAccessor(getter.id()),
                property_origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Property(property.id()),
                property_origin,
            ),
        ];
        foundation.set_definition_origins(origins).unwrap();
        foundation
            .set_types(vec![
                scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
                scoop_identity::CoreBuiltinNominal::Any.identity_record(),
                value_type.clone(),
            ])
            .unwrap();
        foundation.set_properties(vec![property.clone()]).unwrap();
        foundation
            .set_property_accessors(vec![getter.clone()])
            .unwrap();

        let nominal_interface = crate::nominal_interface_fixture::public_record(
            SourceNominalId::Concrete(value_type.id()),
            PublicNominalKindV1::Struct,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            match case {
                ConstSurfaceCase::IntrinsicBoolean => NominalSourceShapeV1::Intrinsic(
                    scoop_hir::NominalIntrinsicRepresentationV1::new(
                        scoop_hir::IntrinsicTypeKind::Boolean,
                    ),
                ),
                ConstSurfaceCase::WrongFamily => NominalSourceShapeV1::Intrinsic(
                    scoop_hir::NominalIntrinsicRepresentationV1::new(
                        scoop_hir::IntrinsicTypeKind::Integer(scoop_hir::IntegerKind::SIGNED_32),
                    ),
                ),
                ConstSurfaceCase::OrdinaryNominal | ConstSurfaceCase::MismatchedOrigin => {
                    NominalSourceShapeV1::Struct(
                        StructSourceShapeV1::try_new(
                            Vec::new(),
                            scoop_hir::NominalCLayoutPolicyV1::Ordinary,
                            false,
                        )
                        .unwrap(),
                    )
                }
            },
        )
        .unwrap();
        let property_interface = PropertyInterfaceRecordV1::try_new(
            PropertyOwner::Property(property.id()),
            PublicDeclarationOwnerV1::TopLevel,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            SignatureTypeKey::Nominal(value_type.id()),
            Accessors::read_only(AccessorSource::new(getter.id(), AccessorForm::Constant)),
            PropertyRepresentationV1::Const,
            PropertyPublicAccessV1::DirectOnly,
            scoop_hir::PropertySetterPublicAccessV1::Restricted,
        )
        .unwrap();
        let getter_interface = CallableInterfaceRecordV1::try_new(
            CallableTemplateOrigin::Accessor(getter.id()),
            PublicDeclarationOwnerV1::TopLevel,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
            SignatureTypeKey::Nominal(value_type.id()),
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
            scoop_hir::CanonicalPersistentIdsV1::empty(),
        )
        .unwrap();
        let constant = ExportConstValueV1::new(
            property.id(),
            SignatureTypeKey::Nominal(value_type.id()),
            CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
            ExportDefinitionSourceV1::new(interface_origin.clone()),
        );
        let mut section = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
            CanonicalNominalInterfacesV1::try_new(vec![nominal_interface]).unwrap(),
            CanonicalCallableInterfacesV1::try_new(vec![getter_interface]).unwrap(),
            CanonicalPropertyInterfacesV1::try_new(vec![property_interface]).unwrap(),
            CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportConstValuesV1::try_new(vec![constant]).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(vec![ExportDefinitionSourceV1::new(
                interface_origin,
            )])
            .unwrap(),
            CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
            Default::default(),
            Default::default(),
        );

        Self {
            cone,
            foundation,
            interface: encode(&section.index_for_wire().unwrap()).unwrap(),
        }
    }

    fn artifact(&self) -> Vec<u8> {
        cross_cone_artifact_for_with_hir_foundation(
            self.cone.clone(),
            Vec::new(),
            &self.foundation,
            self.interface.clone(),
        )
    }
}
