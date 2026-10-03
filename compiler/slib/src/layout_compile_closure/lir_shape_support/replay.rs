use super::SharedLirShapeSupportValidationError as Error;
use scoop_identity::{SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

/// Uses the source roots already checked against shared HIR declarations.
/// Candidate LIR shapes and unrelated identity records cannot add roots.
pub fn replay_shared_mir_shape_support(
    shapes: &mir::CanonicalMirShapeSupportsV1,
    layouts: &lir::CanonicalExactLayoutExportsV1,
    descriptors: &lir::CanonicalExactDescriptorExportsV1,
    identities: &ValidatedIdentityGraph,
    foundation: &lir::ConeLirFoundation,
) -> Result<lir::CanonicalParamFreeShapeSupportExportsV1, Error> {
    let path = WirePath::root();

    if shapes.provider() != foundation.producer() {
        return Err(Error::MirProvider);
    }

    let mut sources = Vec::new();
    scoop_wire::allocation::try_reserve(&mut sources, shapes.records().len(), &path)?;
    for shape in shapes.records() {
        let source = identities.canonical_key::<_, SourceDeclarationKey>(shape.source())?;
        sources.push(source);
    }
    Ok(
        lir::CanonicalParamFreeShapeSupportExportsV1::from_source_refs(
            sources.iter().map(|source| source.as_ref()),
            layouts,
            descriptors,
            foundation,
        )?,
    )
}
