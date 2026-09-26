use scoop_identity::{ConeIdentity, PersistentExactTypeId};
use scoop_lir::*;

/// Test source uses taken from real producer shape records. It authorizes
/// only the LIR harness; it does not stand in for a Compile source reader.
pub(in super::super) struct Source {
    pub(in super::super) roots: Vec<LayoutAbiDependencyV1>,
    pub(in super::super) physical: Vec<(ConeIdentity, ExternalStrongShapeSubjectV1)>,
}

impl Source {
    pub(in super::super) fn from_mir(input: &scoop_mir::SingleConeStrongMirInput) -> Self {
        let mut roots = Vec::new();
        let mut physical = Vec::new();
        for shape in input
            .materialization()
            .dependency_generated_nominal_shapes()
        {
            roots.push(LayoutAbiDependencyV1::new(
                shape.provider(),
                LayoutAbiSemanticTargetV1::ShapeSupport(shape.source()),
            ));
            physical.push((
                shape.provider(),
                ExternalStrongShapeSubjectV1::TypeDescriptor(shape.exact()),
            ));
        }
        roots.sort_unstable();
        roots.dedup();
        physical.sort_unstable();
        physical.dedup();
        Self { roots, physical }
    }

    pub(in super::super) fn new(
        provider: ConeIdentity,
        shapes: &[&ParamFreeShapeSupportExportV1],
        string: PersistentExactTypeId,
    ) -> Self {
        let mut roots = shapes
            .iter()
            .map(|shape| {
                LayoutAbiDependencyV1::new(
                    provider,
                    LayoutAbiSemanticTargetV1::ShapeSupport(shape.source_nominal()),
                )
            })
            .collect::<Vec<_>>();
        roots.push(LayoutAbiDependencyV1::new(
            provider,
            LayoutAbiSemanticTargetV1::Descriptor(string),
        ));
        roots.sort_unstable();
        let mut physical = shapes
            .iter()
            .map(|shape| {
                (
                    provider,
                    ExternalStrongShapeSubjectV1::TypeDescriptor(
                        shape.roles().boxed_value().available().unwrap().exact(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        physical.push((
            provider,
            ExternalStrongShapeSubjectV1::TypeDescriptor(string),
        ));
        physical.sort_unstable();
        Self { roots, physical }
    }

    pub(in super::super) fn imports<'a>(
        &self,
        provider: &ShapeLinkProviderV1<'a>,
        consumer: ConeIdentity,
        definitions: &StrongObjectSymbolSurfaceV1,
    ) -> Vec<ExternalShapeLinkImportV1> {
        self.physical
            .iter()
            .map(|(_, subject)| {
                ExternalShapeLinkImportV1::replay(
                    provider,
                    *subject,
                    consumer,
                    definitions,
                    &NoShapeLinkSupportV1,
                )
                .unwrap()
            })
            .collect()
    }
}

pub(super) fn select<'a>(
    input: &scoop_mir::SingleConeStrongMirInput,
    layout: &'a CrossConeLayoutAbiSectionV1<'a>,
    provider: &ShapeLinkProviderV1<'a>,
) -> StrongProductionDependencySelectionV2<'a> {
    let source = Source::from_mir(input);
    assert!(
        source
            .physical
            .iter()
            .all(|(owner, _)| *owner == layout.provider())
    );
    // The pre-lowering import check has no local LIR definitions yet. The real
    // foundation and every physical use are checked again during publication.
    let foundation =
        OdrFreeLirFoundation::try_new(input.module().cone, CanonicalLirFoundation::empty())
            .unwrap();
    let definitions = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    StrongProductionDependencySelectionV2::try_new(
        input.module().cone,
        layout.target_profile(),
        &[layout.exports()],
        source.imports(provider, input.module().cone, &definitions),
        &source,
    )
    .unwrap()
}

impl LayoutAbiSectionSourceAuthorityV1<()> for Source {
    fn validate_local_exports(&self, exports: &LayoutAbiExportConstituentsV1) -> Result<(), ()> {
        (exports.layouts().records().is_empty()
            && exports.descriptors().records().is_empty()
            && exports.dispatch().records().is_empty()
            && exports.callables().records().is_empty()
            && exports.shape_support().records().is_empty())
        .then_some(())
        .ok_or(())
    }

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&self.roots)
    }

    fn validate_physical_imports(&self, imports: &[ExternalShapeLinkImportV1]) -> Result<(), ()> {
        let actual = imports
            .iter()
            .map(|import| (import.provider(), import.subject()))
            .collect::<Vec<_>>();
        (actual == self.physical).then_some(()).ok_or(())
    }
}
