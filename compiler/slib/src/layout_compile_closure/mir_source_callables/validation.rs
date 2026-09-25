use scoop_hir as hir;
use scoop_identity::DependencyCallableDeclarationId as Declaration;
use scoop_mir as mir;

use super::{
    Error, SharedMirSourceCallableComponent as Component,
    SharedMirSourceCallablePartition as Partition,
};

mod signatures;

/// Reconstructs source function/accessor exports from shared declarations.
/// Constructor/generated, dispatch, initialization and selection joins remain
/// separate obligations of the complete MIR section.
pub fn validate_shared_mir_source_callables(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    inheritance: &hir::CheckedNominalInheritanceGraphV1<'_>,
    ordinary: &mir::CrossConeMirBridgeSectionV1,
    callables: &mir::CanonicalMirCallableBindingsV1,
) -> Result<(), Error> {
    if ordinary.artifact() != source.provider() {
        return Err(Error::Provider {
            expected: source.provider(),
            actual: ordinary.artifact(),
        });
    }
    let metadata = source.metadata();
    let mut expected = hir::select_param_free_source_callables(
        source.provider(),
        metadata.public,
        source.section(),
        metadata.identities,
    )?;
    let classifier = hir::NominalExactLeafClassifierV1::try_from_nominal_interfaces(
        metadata.public.nominal_interfaces().records().iter().chain(
            dependencies
                .iter()
                .flat_map(|dependency| dependency.metadata().public.nominal_interfaces().records()),
        ),
    )
    .map_err(Error::Classifier)?;
    let ordinary_expected = hir::select_ordinary_source_callables(
        source.provider(),
        metadata.public,
        &classifier,
        metadata.identities,
    )?;
    for (&declaration, &source) in &ordinary_expected {
        expected.remove(&declaration);

        Error::require(
            declaration,
            Component::Partition,
            callables.get(declaration.implementation()).is_none(),
        )?;

        let binding = ordinary.export(declaration).ok_or(Error::Missing {
            declaration,
            partition: Partition::Ordinary,
        })?;
        Error::require(
            declaration,
            Component::Implementation,
            binding.implementation() == declaration.implementation(),
        )?;
        signatures::exact(
            declaration,
            metadata,
            source,
            inheritance,
            binding.signature(),
        )?;
    }
    for (&declaration, &source) in &expected {
        Error::require(
            declaration,
            Component::Partition,
            ordinary.export(declaration).is_none(),
        )?;

        let binding = callables
            .get(declaration.implementation())
            .ok_or(Error::Missing {
                declaration,
                partition: Partition::TypeBridge,
            })?;
        signatures::binding(declaration, metadata, source, inheritance, binding)?;
    }
    for binding in ordinary.exports() {
        if !ordinary_expected.contains_key(&binding.declaration()) {
            return Err(Error::Unexpected {
                declaration: binding.declaration(),
                partition: Partition::Ordinary,
            });
        }
    }
    for binding in callables.entries() {
        let declaration = match binding.origin() {
            mir::MirCallableOriginV1::Function(id) => Declaration::Function(*id),
            mir::MirCallableOriginV1::Accessor(id) => Declaration::PropertyAccessor(*id),
            mir::MirCallableOriginV1::Constructor(_)
            | mir::MirCallableOriginV1::Generated { .. } => continue,
        };

        if !expected.contains_key(&declaration) {
            return Err(Error::Unexpected {
                declaration,
                partition: Partition::TypeBridge,
            });
        }
    }
    Ok(())
}
