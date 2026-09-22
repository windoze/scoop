use scoop_wire::{
    BudgetMeter, Encoder, WireEncode, WirePath, domain_separated_cbor_hash,
    domain_separated_cbor_hash_stream_length,
};

use super::{
    EncodeResult, ExternalShapeUndefinedUseV1, LayoutLinkClosureError,
    VerifiedExternalShapeRequirementClosureV1,
};
use crate::link_object::{
    CodeLinkObjectMemberSetV1, VerifiedCodeLinkObjectMemberSetV1,
    VerifiedCurrentConeStrongRelocationClosureV1,
};

const DOMAIN: &str = "scoop-cross-cone-layout-object-coverage-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalShapeRelocationUseSetDigestV1([u8; 32]);

impl ExternalShapeRelocationUseSetDigestV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}
impl WireEncode for ExternalShapeRelocationUseSetDigestV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.bytes(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalShapeObjectCoverageV1 {
    verified_link_objects: CodeLinkObjectMemberSetV1,
    relocation_use_set_digest: ExternalShapeRelocationUseSetDigestV1,
}

impl ExternalShapeObjectCoverageV1 {
    pub const fn verified_link_objects(&self) -> &CodeLinkObjectMemberSetV1 {
        &self.verified_link_objects
    }
    pub const fn relocation_use_set_digest(&self) -> ExternalShapeRelocationUseSetDigestV1 {
        self.relocation_use_set_digest
    }
}
impl WireEncode for ExternalShapeObjectCoverageV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(2)?;
        encoder.field(1)?;
        self.verified_link_objects.encode(encoder)?;
        encoder.field(2)?;
        self.relocation_use_set_digest.encode(encoder)
    }
}

pub(super) fn from_verified<D, C, I>(
    closure: &VerifiedExternalShapeRequirementClosureV1<'_>,
    objects: &VerifiedCodeLinkObjectMemberSetV1<D, C, I>,
    meter: &mut BudgetMeter,
) -> Result<ExternalShapeObjectCoverageV1, LayoutLinkClosureError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    if closure.producer() != objects.producer() {
        return Err(LayoutLinkClosureError::ConsumerMismatch {
            objects: objects.producer(),
            selection: closure.producer(),
        });
    }
    same_relocation_proof(
        closure.legacy_closure().strong_closure(),
        objects
            .final_objects()
            .entry()
            .patch_sites()
            .builtins()
            .strong_relocations(),
        meter,
    )?;
    from_projection(objects.projection(), closure.requirements(), meter)
}

fn from_projection(
    objects: &CodeLinkObjectMemberSetV1,
    requirements: &[ExternalShapeUndefinedUseV1],
    meter: &mut BudgetMeter,
) -> Result<ExternalShapeObjectCoverageV1, LayoutLinkClosureError> {
    let path = WirePath::root();
    meter.check_table_entries(objects.members().len() as u64, &path)?;
    meter.check_table_entries(requirements.len() as u64, &path)?;
    meter.charge_nodes(objects.members().len() as u64, &path)?;
    let depth = u64::from(objects.members().len().max(1).ilog2()) + 1;
    meter.charge_work((requirements.len() as u64).saturating_mul(depth), &path)?;
    for requirement in requirements {
        let member = requirement.use_site().source_member();
        if objects
            .members()
            .binary_search_by_key(&member, |entry| entry.member())
            .is_err()
        {
            return Err(LayoutLinkClosureError::UseOutsideObjectSet { member });
        }
    }
    let digest = digest(objects, requirements, meter)?;
    Ok(ExternalShapeObjectCoverageV1 {
        verified_link_objects: objects.clone(),
        relocation_use_set_digest: digest,
    })
}

/// The final member directory alone cannot attest which pre-patch relocation
/// proof was classified. Bind member contents and every verified resolution.
fn same_relocation_proof(
    classified: &VerifiedCurrentConeStrongRelocationClosureV1,
    finalized: &VerifiedCurrentConeStrongRelocationClosureV1,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutLinkClosureError> {
    let path = WirePath::root();
    meter.check_table_entries(classified.members().len() as u64, &path)?;
    meter.check_table_entries(classified.bindings().len() as u64, &path)?;
    meter.charge_work(1, &path)?;
    if classified.producer() != finalized.producer()
        || classified.members().len() != finalized.members().len()
        || classified.bindings().len() != finalized.bindings().len()
    {
        return Err(LayoutLinkClosureError::ObjectProofMismatch);
    }
    meter.charge_work(classified.members().len() as u64, &path)?;
    for (left, right) in classified.members().iter().zip(finalized.members()) {
        let left_bytes = left.definitions().sections().envelope();
        let right_bytes = right.definitions().sections().envelope();
        if left.member() != right.member()
            || left.producer() != right.producer()
            || left_bytes.byte_length() != right_bytes.byte_length()
            || left_bytes.content_digest() != right_bytes.content_digest()
        {
            return Err(LayoutLinkClosureError::ObjectProofMismatch);
        }
    }
    for (left, right) in classified.bindings().iter().zip(finalized.bindings()) {
        meter.charge_work(
            left.symbol().len() as u64 + right.symbol().len() as u64 + 1,
            &path,
        )?;
        if left != right {
            return Err(LayoutLinkClosureError::ObjectProofMismatch);
        }
    }
    Ok(())
}

struct Preimage<'a> {
    objects: &'a CodeLinkObjectMemberSetV1,
    requirements: &'a [ExternalShapeUndefinedUseV1],
}
impl WireEncode for Preimage<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(2)?;
        encoder.field(1)?;
        self.objects.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.requirements.len() as u64)?;
        for requirement in self.requirements {
            requirement.use_site().encode(encoder)?;
        }
        Ok(())
    }
}

fn digest(
    objects: &CodeLinkObjectMemberSetV1,
    requirements: &[ExternalShapeUndefinedUseV1],
    meter: &mut BudgetMeter,
) -> Result<ExternalShapeRelocationUseSetDigestV1, LayoutLinkClosureError> {
    let path = WirePath::root();
    meter.charge_work(
        objects.members().len() as u64 + requirements.len() as u64,
        &path,
    )?;
    for requirement in requirements {
        meter.charge_work(requirement.use_site().symbol().len() as u64, &path)?;
    }
    let preimage = Preimage {
        objects,
        requirements,
    };
    let stream = domain_separated_cbor_hash_stream_length(DOMAIN, &preimage)?;
    meter.charge_sha256(stream, &path)?;
    let digest = domain_separated_cbor_hash(DOMAIN, &preimage)?;
    Ok(ExternalShapeRelocationUseSetDigestV1(*digest.as_array()))
}

#[cfg(test)]
#[path = "tests/coverage.rs"]
mod tests;
