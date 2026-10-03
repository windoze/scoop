use scoop_identity::{
    ConeIdentity, MangledSymbol, ObjectDefinitionPlanId, PersistentSymbolRequest,
};
use scoop_wire::{Encoder, WireEncode};

use super::wire::{EncodeResult, field};
use super::{ShapeLinkContractV1, ShapeLinkError, ShapeLinkProviderV1};
use crate::ExternalStrongShapeSubjectV1;

mod semantic;
pub(super) use semantic::semantic_target;

/// A provider-bound semantic import. Actual relocation coverage and selection
/// remain obligations of the containing Link/Compile closure.
#[derive(Clone, Debug)]
pub struct ExternalShapeLinkImportV1 {
    pub(super) provider: ConeIdentity,
    pub(super) subject: ExternalStrongShapeSubjectV1,
    pub(super) expected_symbol: PersistentSymbolRequest,
    pub(super) symbol: MangledSymbol,
    pub(super) required_definition: ObjectDefinitionPlanId,
    pub(super) contract: ShapeLinkContractV1,
}

impl ExternalShapeLinkImportV1 {
    pub fn replay<'a>(
        provider: &ShapeLinkProviderV1<'a>,
        subject: ExternalStrongShapeSubjectV1,
        consumer: ConeIdentity,
    ) -> Result<Self, ShapeLinkError> {
        provider.import(subject, consumer)
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
    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }
    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
    pub const fn contract(&self) -> &ShapeLinkContractV1 {
        &self.contract
    }
}

impl WireEncode for ExternalShapeLinkImportV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(5)?;
        field(encoder, 1, &self.provider)?;
        field(encoder, 2, &self.subject)?;
        field(encoder, 3, &self.expected_symbol)?;
        field(encoder, 4, &self.required_definition)?;
        field(encoder, 5, &self.contract)
    }
}
