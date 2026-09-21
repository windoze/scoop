use super::*;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, SignatureTypeKey};

mod provider;
mod selection;
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
        .expand_alias_closure(
            &provider,
            &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
            &scoop_wire::WirePath::root(),
        )
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
    let hir =
        super::super::TrustedCoreBootstrapHirOutput::lower_with_world(&parsed, &world).unwrap();
    let input = hir.machine_input();
    assert_eq!(input.output.imported_dependencies().callable_count(), 1);
    sources::snapshot("hir", &scoop_hir::dump(&input.output.output().export));
    let dependencies = selection::mir(&input);
    let mir = input
        .lower_selected_mir(scoop_mir::CurrentMirProtocolDeclarations, dependencies)
        .unwrap();
    assert_eq!(mir.dependencies.len(), 1);
    assert_eq!(
        mir.strong
            .materialization()
            .imported_dependency_callable_roots()
            .len(),
        1
    );
    sources::snapshot("mir", &scoop_mir::dump(mir.strong.module()));
    let selected = selection::lir(&mir.dependencies);
    let target = scoop_lir::LirTargetProfile::DARWIN_AARCH64;
    let empty =
        scoop_lir::SelectedDependencyLirSet::try_from_callables(ConeIdentity::CORE, Vec::new())
            .unwrap();
    assert!(matches!(
        lower_selected_lir(
            &mir.strong,
            &mir.public,
            scoop_lir_lower::StrongImportedCoreLirInput::Unused,
            &empty,
            target
        ),
        Err(CurrentConeLirStageError::Lowering(
            scoop_lir_lower::StrongLirLoweringError::ImportedDependencyLirCountMismatch {
                mir: 1,
                lir: 0
            }
        ))
    ));
    let (lir, public) = lower_selected_lir(
        &mir.strong,
        &mir.public,
        scoop_lir_lower::StrongImportedCoreLirInput::Unused,
        &selected,
        target,
    )
    .unwrap();
    assert_eq!(lir.module().meta.dependency_external_callables.len(), 1);
    assert_eq!(public.selected().len(), 1);
    assert!(lir.module().meta.core_external_callables.is_empty());
    let dump = scoop_lir::dump(lir.module());
    assert_eq!(dump.matches("dependency-external-fn0").count(), 3);
    sources::snapshot("lir", &dump);
}
