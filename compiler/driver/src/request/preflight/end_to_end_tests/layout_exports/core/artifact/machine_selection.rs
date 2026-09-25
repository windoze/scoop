use scoop_identity::{ConeIdentity, PersistentExactTypeId};
use scoop_lir::*;
use scoop_wire::BudgetMeter;

use super::meter;

/// Test source uses taken from real producer shape records. It authorizes
/// only the LIR harness; it does not stand in for a Compile source reader.
pub(super) struct Source {
    pub(super) roots: Vec<LayoutAbiDependencyV1>,
    pub(super) physical: Vec<(ConeIdentity, ExternalStrongShapeSubjectV1)>,
}

impl Source {
    pub(super) fn new(
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

    pub(super) fn imports<'a>(
        &self,
        provider: &ShapeLinkProviderV1<'a>,
        consumer: ConeIdentity,
        definitions: &StrongObjectSymbolSurfaceV1,
    ) -> Vec<ExternalShapeLinkImportV1<'a>> {
        self.physical
            .iter()
            .map(|(_, subject)| {
                ExternalShapeLinkImportV1::replay(
                    provider,
                    *subject,
                    consumer,
                    definitions,
                    &NoShapeLinkSupportV1,
                    &mut meter(),
                )
                .unwrap()
            })
            .collect()
    }
}

impl LayoutAbiSectionSourceAuthorityV1<()> for Source {
    fn validate_local_exports(
        &self,
        exports: &LayoutAbiExportConstituentsV1,
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
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

    fn validate_physical_imports(
        &self,
        imports: &[ExternalShapeLinkImportV1<'_>],
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        let actual = imports
            .iter()
            .map(|import| (import.provider(), import.subject()))
            .collect::<Vec<_>>();
        (actual == self.physical).then_some(()).ok_or(())
    }
}

pub(super) fn empty_exports(
    foundation: &OdrFreeLirFoundation,
    target: LirTargetProfile,
) -> LayoutAbiExportConstituentsV1 {
    let layouts =
        CanonicalExactLayoutExportsV1::try_new(target, foundation, Vec::new(), &mut meter())
            .unwrap();
    let descriptors =
        CanonicalExactDescriptorExportsV1::try_new(target, foundation, Vec::new(), &mut meter())
            .unwrap();
    let dispatch =
        CanonicalExactDispatchExportsV1::try_new(target, foundation, Vec::new(), &mut meter())
            .unwrap();
    let callables =
        CanonicalExactCallableAbiExportsV1::try_new(target, foundation, Vec::new(), &mut meter())
            .unwrap();
    let shapes = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
        &mut meter(),
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(layouts, descriptors, dispatch, callables, shapes)
        .unwrap()
}
