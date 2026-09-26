use super::*;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, SignatureTypeKey};

mod finite_types;
mod provider;
mod selection;
mod source_types;
mod sources;

#[test]
fn current_core_and_ordinary_callables_share_the_complete_machine_pipeline() {
    let provider = provider::Provider::new(
        "core-helper",
        "run",
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
    );
    let imported = provider.import();
    let aliases = scoop_hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new())
        .unwrap()
        .expand_alias_closure(&[], &scoop_wire::WirePath::root())
        .unwrap();
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ConeIdentity::CORE,
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            provider.certificate(),
            &imported,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let parsed = sources::core_sources();
    let hir = crate::request::preflight::current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        &parsed,
        scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap();
    let input = hir.machine_input();
    assert_eq!(input.output.imported_dependencies().callable_count(), 1);
    sources::snapshot("hir", &scoop_hir::dump(&input.output.output().export));
    let dependencies = selection::mir(&input);
    let mir = input.lower_selected_mir(dependencies).unwrap();
    assert_eq!(mir.selected_callables.len(), 1);
    assert_eq!(
        mir.strong.materialization().external_callable_roots().len(),
        1
    );
    sources::snapshot("mir", &scoop_mir::dump(mir.strong.module()));
    let selected = selection::lir(&mir.selected_callables);
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register_authority(provider.coordinate.identity().unwrap())
        .unwrap();
    hir.foundation.register_identities(&mut pending).unwrap();
    provider
        .foundation
        .register_identities(&mut pending)
        .unwrap();
    mir.strong
        .foundation()
        .as_canonical()
        .register_identities(&mut pending)
        .unwrap();
    let identities = pending.finish().unwrap();
    let coordinates = [
        scoop_identity::ConeCoordinate::reserved_core(),
        provider.coordinate.clone(),
    ];
    let diagnostics =
        scoop_identity::ExactTypeDiagnosticCatalog::try_new(&identities, &coordinates).unwrap();
    let target = scoop_lir::LirTargetProfile::DARWIN_AARCH64;
    let empty =
        scoop_lir::SelectedExternalLirSet::try_from_callables(ConeIdentity::CORE, Vec::new())
            .unwrap();
    let root = &mir.strong.materialization().external_callable_roots()[0];
    assert!(matches!(
        lower_selected_lir(
            &mir.strong,
            &mir.public,
            &empty,
            target,
            &scoop_lir::StrongProductionDependencySelectionV2::empty(ConeIdentity::CORE, target)
                .unwrap(),
            &diagnostics
        ),
        Err(CurrentConeLirStageError::Lowering(
            scoop_lir_lower::StrongLirLoweringError::DependencyLayout(
                scoop_lir::LayoutExternalMaterializationError::MissingCallable {
                    provider,
                    target
                }
            )
        )) if provider == root.provider() && target == root.implementation()
    ));
    let (lir, public) = lower_selected_lir(
        &mir.strong,
        &mir.public,
        &selected,
        target,
        &scoop_lir::StrongProductionDependencySelectionV2::empty(ConeIdentity::CORE, target)
            .unwrap(),
        &diagnostics,
    )
    .unwrap();
    assert_eq!(lir.module().meta.external_callables.len(), 1);
    assert_eq!(public.selected().len(), 1);
    assert!(
        lir.module()
            .meta
            .external_callables
            .iter()
            .all(|(_, callable)| matches!(
                callable.origin(),
                scoop_lir::ExternalCallableOrigin::Legacy(_)
            ))
    );
    let dump = scoop_lir::dump(lir.module());
    assert_eq!(dump.matches("external-fn0").count(), 3);
    sources::snapshot("lir", &dump);
}
