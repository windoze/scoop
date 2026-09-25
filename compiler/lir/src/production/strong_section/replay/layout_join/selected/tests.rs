use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner,
};

struct Source;

impl crate::LayoutAbiSectionSourceAuthorityV1<()> for Source {
    fn validate_local_exports(&self, _: &crate::LayoutAbiExportConstituentsV1) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[crate::LayoutAbiDependencyV1], ()> {
        Ok(&[])
    }

    fn validate_physical_imports(
        &self,
        imports: &[crate::ExternalShapeLinkImportV1<'_>],
    ) -> Result<(), ()> {
        imports.is_empty().then_some(()).ok_or(())
    }
}

#[test]
fn dependency_descriptor_and_dispatch_body_require_selected_physical_closure() {
    let section = empty_section();
    let provider = ConeIdentity::CORE;
    let exact = PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    assert!(matches!(
        descriptor(
            crate::StrongTypeDescriptorRefV2::DependencyExternal { provider, exact },
            Selection::Complete(section.selected())
        ),
        Err(StrongProductionLayoutJoinError::MissingSelectedDescriptor {
            provider: actual,
            exact: actual_exact
        }) if actual == provider && actual_exact == exact
    ));

    let owner = StrongCallableDefinitionOwner::Function(
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            source_site(provider),
            CanonicalIdentifier::new("dispatch").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap(),
    );
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(owner)).unwrap();
    assert!(matches!(
        callable(
            crate::StrongTypeDispatchCallableRefV2::DependencyExternal {
                provider,
                body,
            },
            Selection::Complete(section.selected())
        ),
        Err(StrongProductionLayoutJoinError::MissingPhysicalCallable {
            provider: actual,
            body: actual_body
        }) if actual == provider && actual_body == body
    ));

    let dependency =
        crate::production::initialization_registrations::external_dependency_for_layout_join(
            ConeIdentity::SINGLE_FILE,
        );
    let unit = dependency.unit();
    assert!(matches!(
        initialization(&dependency, Selection::Complete(section.selected())),
        Err(StrongProductionLayoutJoinError::MissingPhysicalInitialization {
            provider: actual,
            unit: actual_unit,
        }) if actual == provider && actual_unit == unit
    ));
}

fn empty_section() -> crate::CrossConeLayoutAbiSectionV1<'static> {
    let foundation = crate::OdrFreeLirFoundation::try_new(
        ConeIdentity::SINGLE_FILE,
        crate::CanonicalLirFoundation::empty(),
    )
    .unwrap();
    let layouts = crate::CanonicalExactLayoutExportsV1::try_new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let descriptors = crate::CanonicalExactDescriptorExportsV1::try_new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let exports = crate::LayoutAbiExportConstituentsV1::try_new(
        layouts.clone(),
        descriptors.clone(),
        crate::CanonicalExactDispatchExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
        )
        .unwrap(),
        crate::CanonicalExactCallableAbiExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
        )
        .unwrap(),
        crate::CanonicalParamFreeShapeSupportExportsV1::from_sources(
            &[],
            &layouts,
            &descriptors,
            &foundation,
        )
        .unwrap(),
    )
    .unwrap();
    crate::CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &Source).unwrap()
}

fn source_site(provider: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
