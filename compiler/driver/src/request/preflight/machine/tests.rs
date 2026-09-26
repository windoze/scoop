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
    let target = scoop_lir::LirTargetProfile::DARWIN_AARCH64;
    let empty =
        scoop_lir::SelectedExternalLirSet::try_from_callables(ConeIdentity::CORE, Vec::new())
            .unwrap();
    assert!(matches!(
        lower_selected_lir(
            &mir.strong,
            &mir.public,
            scoop_lir_lower::RuntimeStringDescriptor::Local,
            &empty,
            target
        ),
        Err(CurrentConeLirStageError::Lowering(
            scoop_lir_lower::StrongLirLoweringError::ExternalCallableCountMismatch {
                mir: 1,
                lir: 0
            }
        ))
    ));
    let (lir, public) = lower_selected_lir(
        &mir.strong,
        &mir.public,
        scoop_lir_lower::RuntimeStringDescriptor::Local,
        &selected,
        target,
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
