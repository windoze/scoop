use scoop_hir::{
    CanonicalBinderListV1, CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalDirectPublicSurfaceV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferencesV1, CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1,
    CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1, CanonicalPublicMemberRefsV1,
    CanonicalSignatureTypesV1, CanonicalTypeAliasInterfacesV1, CoreHirInterfaceBranchV1,
    CrossConeHirInterfaceSectionV1, ExportBindingSourceV1, ExportDefinitionSourceV1,
    HirOutputContractV1, NominalInterfaceRecordV1, NominalSourceShapeV1,
    PublicExportBindingRecordV1, PublicLookupAccessV1, PublicNominalKindV1,
    SignatureTypeSemanticError, SourceNominalId, StructSourceShapeV1, TypeAliasInterfaceRecordV1,
    TypeAliasInterfaceSemanticValidationError, TypeAliasInterfaceSetSemanticValidationError,
    TypeAliasTargetV1,
};
use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CapabilityId, CborIdentityRecord,
    DeclarationScope, DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject,
    DefinitionOwnerChain, ExportBindingKey, NormalizedSourcePath, PackagePath,
    PersistentExportBindingId, PersistentSourceContextId, PersistentTypeAliasId, PersistentTypeId,
    SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::{Encoder, WireEncode, encode};

use super::*;

#[test]
fn validates_a_canonical_type_alias_surface() {
    let fixture = AliasSurface::new(true, true, None);
    let bytes = fixture.artifact();
    let validated = validate_until_callable(&bytes)
        .validate_type_alias_surface(Vec::new())
        .unwrap();

    assert_eq!(validated.identity(), fixture.cone.identity());
    assert!(
        validated
            .hir_interface()
            .type_aliases()
            .get(fixture.alias)
            .is_some()
    );
}

#[test]
fn rejects_a_type_alias_without_a_direct_public_binding() {
    let fixture = AliasSurface::new(false, true, None);
    let bytes = fixture.artifact();
    let front = validate_until_callable(&bytes);

    let Err(CrossConeHirTypeAliasSurfaceError::TypeAliasInterfaces(error)) =
        front.validate_type_alias_surface(Vec::new())
    else {
        panic!("a non-public alias must fail type-alias surface validation");
    };
    assert!(matches!(
        error.as_ref(),
        TypeAliasInterfaceSetSemanticValidationError::Record {
            error: TypeAliasInterfaceSemanticValidationError::Declaration(
                CrossConeHirNominalAuthorityError::MissingDirectPublicTypeAliasBinding { alias }
            ),
            ..
        } if *alias == fixture.alias
    ));
}

#[test]
fn rejects_a_type_alias_definition_origin_mismatch() {
    let fixture = AliasSurface::new(true, false, None);
    let bytes = fixture.artifact();
    let front = validate_until_callable(&bytes);

    let Err(CrossConeHirTypeAliasSurfaceError::TypeAliasInterfaces(error)) =
        front.validate_type_alias_surface(Vec::new())
    else {
        panic!("a mismatched alias origin must fail type-alias surface validation");
    };
    assert!(matches!(
        error.as_ref(),
        TypeAliasInterfaceSetSemanticValidationError::Record {
            error: TypeAliasInterfaceSemanticValidationError::DefinitionOriginMismatch {
                expected,
                actual,
            },
            ..
        } if expected.as_ref() == &fixture.foundation_origin
            && actual.as_ref() == &fixture.interface_origin
    ));
}

#[test]
fn rejects_a_type_alias_target_from_an_unreachable_provider() {
    let fixture = AliasSurface::new(
        true,
        true,
        Some(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ),
    );
    let bytes = fixture.artifact();
    let front = validate_until_callable(&bytes);

    let Err(CrossConeHirTypeAliasSurfaceError::TypeAliasInterfaces(error)) =
        front.validate_type_alias_surface(Vec::new())
    else {
        panic!("an unreachable alias target must fail type-alias surface validation");
    };
    assert!(matches!(
        error.as_ref(),
        TypeAliasInterfaceSetSemanticValidationError::Record {
            error: TypeAliasInterfaceSemanticValidationError::Target(
                SignatureTypeSemanticError::Reference(
                    CrossConeHirNominalAuthorityError::UnreachableProvider {
                        origin: scoop_identity::ConeIdentity::CORE,
                    }
                )
            ),
            ..
        }
    ));
}

#[test]
fn validates_a_type_alias_target_from_a_reachable_provider() {
    let provider_cone = crate::strong_compile_decode::tests::cone_named("alias-target");
    let (foundation, interface, target, _, _, _, _, _) =
        nominal_surface(provider_cone.identity(), true, true, true);
    let provider_bytes = cross_cone_artifact_for_with_hir_foundation(
        provider_cone,
        Vec::new(),
        &foundation,
        interface,
    );
    let mut provider = open_graph(&provider_bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let provider_identities = provider
        .validate_foundation_identities(std::iter::empty())
        .unwrap();

    let fixture = AliasSurface::new(true, true, Some(target));
    let alias_bytes = fixture.artifact();
    let mut alias = open_graph(&alias_bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let alias_identities = alias
        .validate_foundation_identities([&provider_identities])
        .unwrap();
    let provider = provider
        .validate_foundation_structure(provider_identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap();
    let alias = alias
        .validate_foundation_structure(alias_identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(Vec::new())
        .unwrap()
        .validate_type_alias_surface(vec![provider.nominal_provider_view()])
        .unwrap();

    assert!(
        alias
            .hir_interface()
            .type_aliases()
            .get(fixture.alias)
            .is_some()
    );
}

fn validate_until_callable(bytes: &[u8]) -> CallableValidatedCrossConeHirFrontSections<'_> {
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
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(Vec::new())
        .unwrap()
}

struct AliasSurface {
    cone: ConeRecord,
    foundation: CanonicalHirFoundation,
    interface: Vec<u8>,
    direct: Vec<PersistentExportBindingId>,
    alias: PersistentTypeAliasId,
    foundation_origin: DefinitionOrigin,
    interface_origin: DefinitionOrigin,
}

impl AliasSurface {
    fn new(
        alias_is_public: bool,
        matching_origin: bool,
        external_target: Option<PersistentTypeId>,
    ) -> Self {
        let cone = cone();
        let identity = cone.identity();
        let source = SourceIdentity::new(
            identity,
            NormalizedSourcePath::new("src/Alias.scoop").unwrap(),
        )
        .unwrap();
        let context_key = SourceContextKey::File {
            source: source.clone(),
        };
        let context =
            CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context_key.clone())
                .unwrap();
        let foundation_origin =
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 5).unwrap(), &context_key)
                .unwrap();
        let mismatched_origin = DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(6, 11).unwrap(),
            &context_key,
        )
        .unwrap();
        let interface_origin = if matching_origin {
            foundation_origin.clone()
        } else {
            mismatched_origin
        };
        let site = SourceDeclarationSite::new(
            identity,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let nominal_name = CanonicalIdentifier::new("Target").unwrap();
        let nominal =
            CborIdentityRecord::<PersistentTypeId, _>::from_key(SourceDeclarationKey::nominal(
                site.clone(),
                nominal_name.clone(),
                SourceNominalKind::Struct,
                0,
            ))
            .unwrap();
        let alias_name = CanonicalIdentifier::new("Alias").unwrap();
        let alias = CborIdentityRecord::<PersistentTypeAliasId, _>::from_key(
            SourceDeclarationKey::type_alias(site, alias_name.clone()),
        )
        .unwrap();
        let nominal_binding =
            CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
                identity,
                PackagePath::root(),
                nominal_name,
                BindingTarget::type_name(nominal.key()).unwrap(),
            ))
            .unwrap();
        let alias_binding =
            CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
                identity,
                PackagePath::root(),
                alias_name,
                BindingTarget::type_alias(alias.key()).unwrap(),
            ))
            .unwrap();

        let mut foundation = base_hir_foundation();
        foundation
            .set_sources(vec![
                scoop_hir::SourceRecord::from_utf8(
                    source,
                    "typealias Alias = Target",
                    [0, 5, 6, 11],
                )
                .unwrap(),
            ])
            .unwrap();
        foundation.set_source_contexts(vec![context]).unwrap();
        foundation
            .set_definition_origins(vec![
                DefinitionOriginRecord::new(
                    DefinitionOriginSubject::Type(nominal.id()),
                    foundation_origin.clone(),
                ),
                DefinitionOriginRecord::new(
                    DefinitionOriginSubject::TypeAlias(alias.id()),
                    foundation_origin.clone(),
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
        foundation.set_type_aliases(vec![alias.clone()]).unwrap();
        let mut binding_records = vec![nominal_binding.clone()];
        let mut public_bindings = vec![PublicExportBindingRecordV1::new(
            nominal_binding.id(),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::Type(nominal.id()),
            },
        )];
        let mut direct = vec![nominal_binding.id()];
        if alias_is_public {
            binding_records.push(alias_binding.clone());
            public_bindings.push(PublicExportBindingRecordV1::new(
                alias_binding.id(),
                ExportBindingSourceV1::DeclaredCurrent {
                    declaration: BindableEntity::TypeAlias(alias.id()),
                },
            ));
            direct.push(alias_binding.id());
        }
        foundation.set_export_bindings(binding_records).unwrap();

        let nominal_interface = NominalInterfaceRecordV1::try_new(
            SourceNominalId::Concrete(nominal.id()),
            PublicNominalKindV1::Struct,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            NominalSourceShapeV1::Struct(StructSourceShapeV1::try_new(Vec::new()).unwrap()),
        )
        .unwrap();
        let definition_source = ExportDefinitionSourceV1::new(interface_origin.clone());
        let alias_interface = TypeAliasInterfaceRecordV1::try_new(
            alias.id(),
            TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(
                external_target.unwrap_or_else(|| nominal.id()),
            )),
            PublicLookupAccessV1::DirectOnly,
            definition_source.clone(),
        )
        .unwrap();
        let mut section = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(public_bindings).unwrap(),
            CanonicalNominalInterfacesV1::try_new(vec![nominal_interface]).unwrap(),
            CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalTypeAliasInterfacesV1::try_new(vec![alias_interface]).unwrap(),
            CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(vec![definition_source]).unwrap(),
            CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
        );

        Self {
            cone,
            foundation,
            interface: encode(&section.index_for_wire().unwrap()).unwrap(),
            direct,
            alias: alias.id(),
            foundation_origin,
            interface_origin,
        }
    }

    fn artifact(&self) -> Vec<u8> {
        let (mut hir, mut mir, mut lir) = required_sections();
        retarget_lir_sections(&self.cone, &mut lir);
        replace_section(
            &mut hir,
            hir_identity_foundation_capability(),
            encode(&self.foundation).unwrap(),
        );
        let direct = CanonicalDirectPublicSurfaceV1::try_new(self.direct.clone()).unwrap();
        replace_section(
            &mut hir,
            hir_core_bootstrap_interface_capability(),
            encode(&NonCoreHirProduction { direct: &direct }).unwrap(),
        );
        hir.push(section(
            MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
            MemberPurposeSet::COMPILE,
            self.interface.clone(),
        ));
        add_cross_cone_bridge_sections(&mut mir, &mut lir);
        build_artifact_for_profile_with_dependencies(
            self.cone.clone(),
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            Vec::new(),
            hir,
            mir,
            lir,
            false,
        )
    }
}

fn replace_section(
    sections: &mut [crate::MetadataSection],
    capability: CapabilityId,
    bytes: Vec<u8>,
) {
    let existing = sections
        .iter_mut()
        .find(|section| section.capability() == &capability)
        .expect("the shared fixture has the required HIR section");
    *existing = section(
        MetadataLocation::Hir,
        capability,
        MemberPurposeSet::COMPILE,
        bytes,
    );
}

struct NonCoreHirProduction<'a> {
    direct: &'a CanonicalDirectPublicSurfaceV1,
}

impl WireEncode for NonCoreHirProduction<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        CoreHirInterfaceBranchV1::NotCore.encode(encoder)?;
        encoder.field(2)?;
        HirOutputContractV1::Library.encode(encoder)?;
        encoder.field(3)?;
        self.direct.encode(encoder)
    }
}
