use super::*;

mod enums;
mod nominals;
pub(super) use enums::enum_variant_callable_surface;
pub(super) use nominals::nominal_surface;

pub(super) fn builtin_provider_artifact() -> Vec<u8> {
    let provider = ConeRecord::new(
        scoop_identity::ConeCoordinate::reserved_core(),
        crate::ConeKind::Library,
        crate::ConeSourceForm::Manifest,
    )
    .unwrap();
    cross_cone_artifact_for_with_hir_foundation(
        provider,
        vec![],
        &base_hir_foundation(),
        empty_cross_cone_hir_interface(),
    )
}

pub(super) fn declaration_front(
    bytes: &[u8],
) -> crate::HirProductionValidatedCrossConeHirFrontSections<'_> {
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
}

pub(super) fn base_hir_foundation() -> CanonicalHirFoundation {
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    foundation
}

pub(super) fn interface_with_nominals(records: Vec<NominalInterfaceRecordV1>) -> Vec<u8> {
    interface_with_declarations(records, Vec::new(), Vec::new())
}

pub(super) fn interface_with_declarations(
    nominals: Vec<NominalInterfaceRecordV1>,
    callables: Vec<CallableInterfaceRecordV1>,
    properties: Vec<PropertyInterfaceRecordV1>,
) -> Vec<u8> {
    interface_with_support(nominals, callables, properties, Vec::new())
}

fn interface_with_support(
    nominals: Vec<NominalInterfaceRecordV1>,
    callables: Vec<CallableInterfaceRecordV1>,
    properties: Vec<PropertyInterfaceRecordV1>,
    support: Vec<scoop_hir::CallableDeclarationRecordV1>,
) -> Vec<u8> {
    let mut section = CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(nominals).unwrap(),
        CanonicalCallableInterfacesV1::with_support(callables, support).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(properties).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    );
    encode(&section.index_for_wire().unwrap()).unwrap()
}

pub(super) fn scoop_effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}
