use scoop_identity::{ConeIdentity, PersistentExactTypeId};
use scoop_lir::*;

/// Typed uses taken from actual source materializations.
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
        for source in input.module().meta.source_exact_types.iter() {
            if !matches!(
                source.ty(),
                scoop_mir::Type::String | scoop_mir::Type::Class(_) | scoop_mir::Type::Interface(_)
            ) {
                continue;
            }
            let scoop_mir::SourceExactTypeOwner::Cone(provider) = source.owner() else {
                continue;
            };
            if provider == input.module().cone {
                continue;
            }
            let exact = source.identity_record().id();
            roots.push(LayoutAbiDependencyV1::new(
                provider,
                LayoutAbiSemanticTargetV1::Descriptor(exact),
            ));
            physical.push((
                provider,
                ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
            ));
            if source.ty() == &scoop_mir::Type::String
                && !input.materialization().strings().is_empty()
            {
                physical.push((
                    provider,
                    ExternalStrongShapeSubjectV1::TypeRegistration(exact),
                ));
            }
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
    ) -> Vec<ExternalShapeLinkImportV1> {
        self.physical
            .iter()
            .map(|(_, subject)| {
                ExternalShapeLinkImportV1::replay(provider, *subject, consumer).unwrap()
            })
            .collect()
    }
}

pub(super) fn select<'a>(
    input: &scoop_mir::SingleConeStrongMirInput,
    layout: &'a LayoutAbiExportConstituentsV1,
    provider: &ShapeLinkProviderV1<'a>,
) -> StrongProductionDependencySelectionV2<'a> {
    let source = Source::from_mir(input);
    assert!(
        source
            .physical
            .iter()
            .all(|(owner, _)| *owner == layout.provider())
    );
    StrongProductionDependencySelectionV2::try_new(
        input.module().cone,
        layout.target_profile(),
        &[layout],
        source.imports(provider, input.module().cone),
        &source.roots,
    )
    .unwrap()
}
