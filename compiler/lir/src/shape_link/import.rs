use scoop_identity::{ConeIdentity, ObjectDefinitionPlanId, PersistentSymbolRequest};
use scoop_wire::{Encoder, WireEncode};

use super::wire::{EncodeResult, field};
use super::{ShapeLinkContractV1, ShapeLinkError, ShapeLinkProviderV1, ShapeLinkSupportLookupV1};
use crate::{ExternalStrongShapeSubjectV1, StrongObjectSymbolSurfaceV1};

mod semantic;
pub(super) use semantic::semantic_target;

/// A provider-bound semantic import. Actual relocation coverage and selection
/// remain obligations of the containing Link/Compile closure.
#[derive(Clone, Copy, Debug)]
pub struct ExternalShapeLinkImportV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) subject: ExternalStrongShapeSubjectV1,
    pub(super) expected_symbol: PersistentSymbolRequest,
    pub(super) required_definition: ObjectDefinitionPlanId,
    pub(super) contract: ShapeLinkContractV1<'a>,
}

impl<'a> ExternalShapeLinkImportV1<'a> {
    pub fn replay(
        provider: &ShapeLinkProviderV1<'a>,
        subject: ExternalStrongShapeSubjectV1,
        consumer: ConeIdentity,
        consumer_definitions: &StrongObjectSymbolSurfaceV1,
        support: &dyn ShapeLinkSupportLookupV1<'a>,
    ) -> Result<Self, ShapeLinkError> {
        provider.import(subject, consumer, consumer_definitions, support)
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
    pub const fn subject(&self) -> ExternalStrongShapeSubjectV1 {
        self.subject
    }
    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }
    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
    pub const fn contract(&self) -> &ShapeLinkContractV1<'a> {
        &self.contract
    }
}

impl WireEncode for ExternalShapeLinkImportV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(5)?;
        field(encoder, 1, &self.provider)?;
        field(encoder, 2, &self.subject)?;
        field(encoder, 3, &self.expected_symbol)?;
        field(encoder, 4, &self.required_definition)?;
        field(encoder, 5, &self.contract)
    }
}
