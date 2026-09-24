use super::*;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    expected: &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
) {
    let local = &input.hir.output().local;
    let exact = local.exact_type_identities[local.string].clone();
    let scoop_identity::ExactTypeKey::Nominal(owner) = exact.key() else {
        panic!("String has a source nominal owner")
    };
    let provider = input
        .identities
        .canonical_key::<_, scoop_identity::SourceDeclarationKey>(*owner)
        .unwrap()
        .origin();
    let (mut module, selected) = scoop_mir_lower::lower_current_cone(
        input.hir,
        mir::SelectedExternalMirSet::empty(local.cone),
    )
    .unwrap()
    .into_parts();
    assert!(
        module
            .meta
            .source_exact_types
            .get(&mir::Type::String)
            .is_none()
    );
    let mut identities = module
        .meta
        .source_exact_types
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    identities.push(
        mir::SourceExactTypeIdentity::checked(
            mir::Type::String,
            exact,
            mir::SourceExactTypeOrigin::Nominal(provider),
        )
        .unwrap(),
    );
    module.meta.source_exact_types = mir::SourceExactTypeIdentities::checked(identities).unwrap();
    // An otherwise valid MIR lookup entry does not create a source use.
    module.validate().unwrap();
    let foundation = mir::OdrFreeMirFoundation::from_module(&module).unwrap();
    let polluted = mir::SingleConeStrongMirInput::try_new(
        module,
        foundation,
        input.mir.production().clone(),
        vec![],
        mir::StrongExternalCallableInput::Selected(&selected),
    )
    .unwrap();
    let types = dependencies::mir_types(&polluted, input.identities);
    let actual = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
        scoop_mir_lower::MirTypeBridgeExportInputV1 {
            mir: &polluted,
            ..input
        },
        scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
            types: &[&types],
            callables: &[],
            dispatch: &[],
        },
        mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter()).unwrap(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(
        actual.committed_external_uses().unwrap(),
        expected.committed_external_uses().unwrap()
    );
}
