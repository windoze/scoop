use scoop_hir as hir;
use scoop_identity::DependencyCallableDeclarationId as Declaration;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

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
    meter: &mut BudgetMeter,
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
        meter,
    )?;
    let classifier = hir::NominalExactLeafClassifierV1::try_from_nominal_interfaces_metered(
        metadata.public.nominal_interfaces().records().iter().chain(
            dependencies
                .iter()
                .flat_map(|dependency| dependency.metadata().public.nominal_interfaces().records()),
        ),
        meter,
        &WirePath::root(),
    )
    .map_err(Error::Classifier)?;
    let ordinary_expected = hir::select_ordinary_source_callables(
        source.provider(),
        metadata.public,
        &classifier,
        metadata.identities,
        meter,
    )?;
    for (&declaration, &source) in &ordinary_expected {
        lookup(expected.len(), meter)?;
        expected.remove(&declaration);
        lookup(callables.entries().len(), meter)?;
        Error::require(
            declaration,
            Component::Partition,
            callables.get(declaration.implementation()).is_none(),
        )?;
        lookup(ordinary.exports().len(), meter)?;
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
            meter,
        )?;
    }
    for (&declaration, &source) in &expected {
        lookup(ordinary.exports().len(), meter)?;
        Error::require(
            declaration,
            Component::Partition,
            ordinary.export(declaration).is_none(),
        )?;
        lookup(callables.entries().len(), meter)?;
        let binding = callables
            .get(declaration.implementation())
            .ok_or(Error::Missing {
                declaration,
                partition: Partition::TypeBridge,
            })?;
        signatures::binding(declaration, metadata, source, inheritance, binding, meter)?;
    }
    for binding in ordinary.exports() {
        lookup(ordinary_expected.len(), meter)?;
        if !ordinary_expected.contains_key(&binding.declaration()) {
            return Err(Error::Unexpected {
                declaration: binding.declaration(),
                partition: Partition::Ordinary,
            });
        }
    }
    for binding in callables.entries() {
        meter.charge_work(1, &WirePath::root())?;
        let declaration = match binding.origin() {
            mir::MirCallableOriginV1::Function(id) => Declaration::Function(*id),
            mir::MirCallableOriginV1::Accessor(id) => Declaration::PropertyAccessor(*id),
            mir::MirCallableOriginV1::Constructor(_)
            | mir::MirCallableOriginV1::Generated { .. } => continue,
        };
        lookup(expected.len(), meter)?;
        if !expected.contains_key(&declaration) {
            return Err(Error::Unexpected {
                declaration,
                partition: Partition::TypeBridge,
            });
        }
    }
    Ok(())
}

fn lookup(length: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?)
}
