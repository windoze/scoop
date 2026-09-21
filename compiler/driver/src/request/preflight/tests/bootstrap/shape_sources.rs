use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_mir::{CoreShapeSupportSourceInput, SingleConeStrongMirInputError as Error};

#[test]
fn shape_demands_are_validated_against_real_mir_without_a_production_root_copy() {
    let root = crate::workspace_root().join("sysroot/lib/scoop.core");
    let manifest =
        scoop_manifest::load_cone_manifest(&ManifestRootLocator::cone_directory(root)).unwrap();
    let sources = discover_manifest_sources(&manifest).unwrap();
    let parsed = parse_discovered_sources(&sources).unwrap();
    let hir = TrustedCoreBootstrapHirOutput::lower(&parsed).unwrap();
    let scoop_hir::LocalConcreteMaterializationContract::CoreShapeSupport(plan) =
        hir.hir().local.materialization()
    else {
        panic!("the core HIR graph carries its complete shape demand")
    };
    let sources = plan
        .roots()
        .iter()
        .map(|root| root.declaration().clone())
        .collect::<Vec<_>>();
    assert!(sources.len() > 1);
    let first = plan.roots()[0].source();
    let second = plan.roots()[1].source();
    for (sources, previous, current) in [
        (vec![sources[0].clone(), sources[0].clone()], first, first),
        (vec![sources[1].clone(), sources[0].clone()], second, first),
    ] {
        assert!(matches!(seal(&hir, sources, |_| {}),
            Err(Error::NonCanonicalCoreShapeSupportSource { index: 1, previous: actual_previous, current: actual_current })
                if actual_previous == previous && actual_current == current
        ));
    }
    for (provider, parameters) in [(ConeIdentity::SINGLE_FILE, 0), (ConeIdentity::CORE, 1)] {
        assert!(matches!(
            seal(&hir, vec![declaration(provider, parameters)], |_| {}),
            Err(Error::InvalidCoreShapeSupportSource { index: 0 })
        ));
    }
    let absent = declaration(ConeIdentity::CORE, 0);
    let expected_source = PersistentTypeId::from_source_declaration(&absent).unwrap();
    let expected_exact =
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(expected_source)).unwrap();
    assert!(matches!(seal(&hir, vec![absent], |_| {}),
        Err(Error::MissingCoreShapeSupportSource { source, exact }) if source == expected_source && exact == expected_exact
    ));
    for mutate in [
        |module: &mut scoop_mir::Module| module.meta.coroutine_steps.clear(),
        |module: &mut scoop_mir::Module| module.meta.coroutine_slots.clear(),
        |module: &mut scoop_mir::Module| module.meta.boxed_types.clear(),
    ] {
        let Err(Error::Foundation(scoop_mir::MirFoundationBuildError::InvalidModule(error))) =
            seal(&hir, sources.clone(), mutate)
        else {
            panic!("incomplete helper metadata must fail the common foundation validator")
        };
        assert!(matches!(
            error.location,
            scoop_mir::MirValidationLocation::GeneratedExactType { .. }
        ));
        assert_eq!(
            error.kind,
            scoop_mir::MirValidationErrorKind::InvalidGeneratedExactType {
                reason: "the exact-type relation contains an unclaimed generated nominal",
            }
        );
    }
}

fn declaration(provider: ConeIdentity, parameters: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("MissingShapeSource").unwrap(),
        SourceNominalKind::Struct,
        parameters,
    )
}

fn seal(
    hir: &TrustedCoreBootstrapHirOutput,
    sources: Vec<SourceDeclarationKey>,
    mutate: impl FnOnce(&mut scoop_mir::Module),
) -> Result<scoop_mir::SingleConeStrongMirInput, Error> {
    let mut module = scoop_mir_lower::lower(&hir.hir().local).unwrap();
    mutate(&mut module);
    let canonical =
        scoop_mir::CanonicalMirFoundation::from_module(&module).map_err(Error::Foundation)?;
    let foundation = scoop_mir::OdrFreeMirFoundation::try_new(canonical).unwrap();
    let production = scoop_mir_lower::lower_production_section(
        module.cone,
        hir.production_section(),
        &foundation,
    )
    .unwrap();
    scoop_mir::SingleConeStrongMirInput::try_new(
        module,
        foundation,
        production,
        CoreShapeSupportSourceInput::Core(sources),
        scoop_mir::StrongImportedCoreInput::Unused,
    )
}
