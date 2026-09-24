use super::SharedLirShapeSupportValidationError as Error;
use scoop_identity::{SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

/// Uses the source roots already checked against shared HIR declarations.
/// Candidate LIR shapes and unrelated identity records cannot add roots.
pub fn replay_shared_mir_shape_support(
    shapes: &mir::CanonicalMirShapeSupportsV1,
    layouts: &lir::CanonicalExactLayoutExportsV1,
    descriptors: &lir::CanonicalExactDescriptorExportsV1,
    identities: &ValidatedIdentityGraph,
    foundation: &lir::OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalParamFreeShapeSupportExportsV1, Error> {
    let path = WirePath::root();
    meter.charge_work(1, &path)?;
    if shapes.provider() != foundation.producer() {
        return Err(Error::MirProvider);
    }
    meter.check_table_entries(shapes.records().len() as u64, &path)?;
    let mut sources = Vec::new();
    meter.try_reserve_collection_slots(&mut sources, shapes.records().len(), &path)?;
    for shape in shapes.records() {
        meter.charge_work(1, &path)?;
        let source = identities.canonical_key::<_, SourceDeclarationKey>(shape.source())?;
        sources.push(source);
    }
    Ok(
        lir::CanonicalParamFreeShapeSupportExportsV1::from_source_refs(
            sources.iter().map(|source| source.as_ref()),
            layouts,
            descriptors,
            foundation,
            meter,
        )?,
    )
}
