//! General shape imports and their verified Link-only relocation coverage.

use scoop_identity::ConeIdentity;
use scoop_lir::CanonicalExternalShapeLinkImportsV1;
use scoop_wire::{BudgetMeter, Encoder, WireEncode};

use super::{CanonicalUndefinedRelocationUseV1, VerifiedCodeLinkObjectMemberSetV1};

mod classification;
mod coverage;
mod error;
mod terminal;
mod wire;

pub use classification::*;
pub use coverage::{ExternalShapeObjectCoverageV1, ExternalShapeRelocationUseSetDigestV1};
pub use error::LayoutLinkClosureError;
pub use terminal::*;
pub use wire::DecodedCrossConeLayoutLinkClosureSectionV1;

type EncodeResult = Result<(), scoop_wire::cbor::EncodeError>;

/// A canonical actual use, indexed into the unchanged Compile import array.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalShapeUndefinedUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    import_index: u32,
}

impl ExternalShapeUndefinedUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }
    pub const fn import_index(&self) -> u32 {
        self.import_index
    }
}

impl WireEncode for ExternalShapeUndefinedUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(2)?;
        encoder.field(1)?;
        self.use_site.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.import_index))
    }
}

/// The complete new Link constituent. The borrowed import table is exactly
/// the Compile selection used during classification; it is never filtered.
#[derive(Clone, Debug)]
pub struct CrossConeLayoutLinkClosureSectionV1<'a> {
    closure: &'a VerifiedExternalShapeRequirementClosureV1<'a>,
    object_coverage: ExternalShapeObjectCoverageV1,
}

impl<'a> CrossConeLayoutLinkClosureSectionV1<'a> {
    pub fn from_verified_requirements<D, C, I>(
        closure: &'a VerifiedExternalShapeRequirementClosureV1<'a>,
        objects: &VerifiedCodeLinkObjectMemberSetV1<D, C, I>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, LayoutLinkClosureError>
    where
        D: scoop_lir::StrongDescriptorReference,
        C: Clone,
    {
        let object_coverage = coverage::from_verified(closure, objects, meter)?;
        Ok(Self {
            closure,
            object_coverage,
        })
    }
    pub const fn consumer(&self) -> ConeIdentity {
        self.closure.producer()
    }
    pub const fn semantic_imports(&self) -> &'a CanonicalExternalShapeLinkImportsV1<'a> {
        self.closure.semantic_imports()
    }
    pub fn requirements(&self) -> &'a [ExternalShapeUndefinedUseV1] {
        self.closure.requirements()
    }
    pub const fn object_coverage(&self) -> &ExternalShapeObjectCoverageV1 {
        &self.object_coverage
    }
}

impl WireEncode for CrossConeLayoutLinkClosureSectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_imports().encode(encoder)?;
        encoder.field(2)?;
        sequence(encoder, self.requirements())?;
        encoder.field(3)?;
        self.object_coverage.encode(encoder)
    }
}

fn sequence(encoder: &mut Encoder, values: &[impl WireEncode]) -> EncodeResult {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::link_object) mod tests;
