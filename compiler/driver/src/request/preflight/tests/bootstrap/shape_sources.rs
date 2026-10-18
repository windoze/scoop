use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_mir::ConeMirInputError as Error;

#[test]
fn shape_demands_are_validated_against_real_mir_without_a_production_root_copy() {
    let root = crate::workspace_root().join("sysroot/src/scoop.core");
    let manifest =
        scoop_toolchain::load_cone_manifest(&ManifestRootLocator::cone_directory(root)).unwrap();
    let sources =
        discover_manifest_sources(&manifest, scoop_target_data::DARWIN_AARCH64_ID).unwrap();
    let parsed = parse_discovered_sources(&sources).unwrap();
    let world =
        scoop_hir::ImportedSemanticWorld::from_dependencies(parsed.cone(), Vec::new(), Vec::new())
            .unwrap();
    let hir = crate::request::preflight::current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        &parsed,
        scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
        &world,
        Default::default(),
    )
    .unwrap();
    let plan = hir.hir.output().local.materialization();
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
        assert!(matches!(seal(&hir, sources),
            Err(Error::NonCanonicalShapeSupportSource { index: 1, previous: actual_previous, current: actual_current })
                if actual_previous == previous && actual_current == current
        ));
    }
    for (provider, parameters) in [(ConeIdentity::SINGLE_FILE, 0), (ConeIdentity::CORE, 1)] {
        assert!(matches!(
            seal(&hir, vec![declaration(provider, parameters)]),
            Err(Error::InvalidShapeSupportSource { index: 0 })
        ));
    }
    let absent = declaration(ConeIdentity::CORE, 0);
    let expected_source = PersistentTypeId::from_source_declaration(&absent).unwrap();
    let expected_exact =
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(expected_source)).unwrap();
    assert!(matches!(seal(&hir, vec![absent]),
        Err(Error::MissingShapeSupportSource { source, exact }) if source == expected_source && exact == expected_exact
    ));
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
    hir: &crate::request::preflight::current_hir::CurrentConeHirArtifacts,
    sources: Vec<SourceDeclarationKey>,
) -> Result<scoop_mir::ConeMirInput, Error> {
    let module = scoop_mir_lower::lower(&hir.hir.output().local).unwrap();
    let selected = scoop_mir::SelectedExternalMirSet::empty(module.cone);
    let output = scoop_mir::DependencyMirOutput::try_new(module, selected).unwrap();
    let foundation = output.foundation();
    let production = scoop_mir_lower::lower_production_section(
        output.module().cone,
        &hir.production_section,
        foundation,
    )
    .unwrap();
    scoop_mir::ConeMirInput::try_new(output, production, sources)
}
