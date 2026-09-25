use scoop_identity::*;
use scoop_lir::*;
use scoop_wire::{decode_canonical, encode};

pub(in crate::link_object::layout_link_closure) const TARGET: LirTargetProfile =
    LirTargetProfile::DARWIN_AARCH64;

pub(in crate::link_object) struct Provider {
    pub foundation: OdrFreeLirFoundation,
    pub production: ValidatedStrongProductionSectionV2,
    pub ordinary: CrossConeLirBridgeSectionV1,
    pub section: CrossConeLayoutAbiSectionV1<'static>,
    pub layout: PersistentLayoutId,
}

impl Provider {
    pub fn new() -> Self {
        let coordinate = ConeCoordinate::new("test", "layout-link-provider", "1.0.0").unwrap();
        let provider = coordinate.identity().unwrap();
        let (mut canonical, image) =
            crate::link_decode::tests::strong_production_fixture(coordinate.clone(), &[]);
        let source = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Word").unwrap(),
            SourceNominalKind::Struct,
            0,
        );
        let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            PersistentTypeId::from_source_declaration(&source).unwrap(),
        ))
        .unwrap();
        let layout = CborIdentityRecord::from_key(LayoutKey::new(
            exact.id(),
            TARGET.wire_id(),
            RepresentationRole::ManagedValue,
        ))
        .unwrap();
        let scan =
            CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::InlineValue)).unwrap();
        let image_plan = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                provider,
                StrongDefinitionEntity::cone_image(provider),
                StrongDefinitionRole::ImageDescriptor,
            )
            .unwrap(),
        )
        .unwrap();
        let mut atoms = crate::link_decode::tests::image_atoms(image_plan.id());
        let mut plans = vec![image_plan];
        let mut symbols = vec![
            PersistentSymbolRequest::new(
                PersistentSymbolKey::ImageDescriptor(provider),
                LinkageClass::ConeStrong,
            )
            .unwrap(),
        ];
        for subject in [
            ExternalStrongShapeSubjectV1::Layout(layout.id()),
            ExternalStrongShapeSubjectV1::Scan(scan.id()),
        ] {
            let (plan, symbol) = subject.expected_definition(provider).unwrap();
            let plan = CborIdentityRecord::from_key(plan).unwrap();
            atoms.push(
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    plan.id(),
                    DefinitionAtomRole::Primary,
                    DefinitionAtomSubkey::Singleton,
                ))
                .unwrap(),
            );
            plans.push(plan);
            symbols.push(PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap());
        }
        canonical.set_layouts(vec![layout.clone()]).unwrap();
        canonical.set_scans(vec![scan]).unwrap();
        canonical.set_definition_plans(plans).unwrap();
        canonical.set_definition_atoms(atoms).unwrap();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
        let foundation = OdrFreeLirFoundation::try_new(provider, canonical).unwrap();
        let identity = ExactLayoutIdentityV1::from_foundation(
            TARGET,
            exact,
            RepresentationRole::ManagedValue,
            &foundation,
        )
        .unwrap();
        let value = ExactValueLayoutV1::scalar(
            identity,
            ScalarRepresentationKindV1::Integer(IntegerKind::SIGNED_64),
            &foundation,
        )
        .unwrap();
        let exports = exports(&foundation, vec![value.into()]);
        let source = Source::local(&exports);
        let section = CrossConeLayoutAbiSectionV1::try_new(exports, &[], vec![], &source).unwrap();
        let external = StrongExternalLirBridgeSurfaceV1::try_new(provider, Vec::new()).unwrap();
        let digests = image.digest_finalization_plan().clone();
        let old = StrongProductionSectionV1::new(
            coordinate.clone(),
            &[scoop_identity::ConeIdentity::CORE],
            &foundation,
            external.clone(),
            digests.clone(),
            StrongRegistrationProductionSurfaceV1::empty(TARGET, &foundation, &digests).unwrap(),
            EntryProductionSourceV1::Library,
            &[],
            None,
        )
        .unwrap();
        let raw: DecodedStrongProductionSectionV2 =
            decode_canonical(&encode(&old).unwrap()).unwrap();
        let production = raw
            .replay(
                coordinate,
                old.image_plan().dependencies(),
                TARGET,
                &foundation,
                external,
                EntryProductionSourceV1::Library,
                &[],
                None,
                &StrongTypeReferenceDefinitionsV2::new(provider, &[]).unwrap(),
                &StrongInitializationDefinitionCatalogV2::new(provider, &[]).unwrap(),
            )
            .unwrap()
            .validate_layout_abi(&section)
            .unwrap();
        let ordinary =
            CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), Vec::new()).unwrap();
        Self {
            foundation,
            production,
            ordinary,
            section,
            layout: layout.id(),
        }
    }

    pub fn import(&self, consumer: ConeIdentity) -> ExternalShapeLinkImportV1<'_> {
        let provider = ShapeLinkProviderV1::try_new(ShapeLinkProviderPartsV1 {
            foundation: &self.foundation,
            production: ShapeLinkProductionV1::Reader(&self.production),
            ordinary: &self.ordinary,
            layouts: self.section.layouts(),
            callables: self.section.callables(),
            descriptors: self.section.descriptors(),
            dispatch: self.section.dispatch(),
        })
        .unwrap();
        let foundation =
            OdrFreeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
        ExternalShapeLinkImportV1::replay(
            &provider,
            ExternalStrongShapeSubjectV1::Layout(self.layout),
            consumer,
            &StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap(),
            &NoShapeLinkSupportV1,
        )
        .unwrap()
    }

    pub fn consumer(&self, consumer: ConeIdentity) -> CrossConeLayoutAbiSectionV1<'_> {
        let foundation =
            OdrFreeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
        let exports = exports(&foundation, vec![]);
        let import = self.import(consumer);
        let mut source = Source::local(&exports);
        source.roots.push(LayoutAbiDependencyV1::new(
            self.foundation.producer(),
            LayoutAbiSemanticTargetV1::Layout(self.layout),
        ));
        source.imports.push(encode(&import).unwrap());
        CrossConeLayoutAbiSectionV1::try_new(exports, &[&self.section], vec![import], &source)
            .unwrap()
    }
}

pub(in crate::link_object) fn empty_section(
    provider: ConeIdentity,
) -> CrossConeLayoutAbiSectionV1<'static> {
    let foundation =
        OdrFreeLirFoundation::try_new(provider, CanonicalLirFoundation::empty()).unwrap();
    let exports = exports(&foundation, vec![]);
    let source = Source::local(&exports);
    CrossConeLayoutAbiSectionV1::try_new(exports, &[], vec![], &source).unwrap()
}

pub(in crate::link_object::layout_link_closure) fn exports(
    foundation: &OdrFreeLirFoundation,
    values: Vec<ExactLayoutExportV1>,
) -> LayoutAbiExportConstituentsV1 {
    let layouts = CanonicalExactLayoutExportsV1::try_new(TARGET, foundation, values).unwrap();
    let descriptors =
        CanonicalExactDescriptorExportsV1::try_new(TARGET, foundation, vec![]).unwrap();
    let dispatch = CanonicalExactDispatchExportsV1::try_new(TARGET, foundation, vec![]).unwrap();
    let callables =
        CanonicalExactCallableAbiExportsV1::try_new(TARGET, foundation, vec![]).unwrap();
    let support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(layouts, descriptors, dispatch, callables, support)
        .unwrap()
}

/// The fixture records its actual source layout and committed import request
/// before section construction; the section still performs every graph join.
struct Source {
    roots: Vec<LayoutAbiDependencyV1>,
    exports: LayoutAbiExportConstituentsV1,
    imports: Vec<Vec<u8>>,
}
impl Source {
    fn local(exports: &LayoutAbiExportConstituentsV1) -> Self {
        Self {
            roots: vec![],
            exports: exports.clone(),
            imports: vec![],
        }
    }
}
impl LayoutAbiSectionSourceAuthorityV1<()> for Source {
    fn validate_local_exports(&self, exports: &LayoutAbiExportConstituentsV1) -> Result<(), ()> {
        (*exports == self.exports).then_some(()).ok_or(())
    }
    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&self.roots)
    }
    fn validate_physical_imports(
        &self,
        imports: &[ExternalShapeLinkImportV1<'_>],
    ) -> Result<(), ()> {
        (imports.len() == self.imports.len()
            && imports
                .iter()
                .zip(&self.imports)
                .all(|(actual, expected)| encode(actual).unwrap() == *expected))
        .then_some(())
        .ok_or(())
    }
}
