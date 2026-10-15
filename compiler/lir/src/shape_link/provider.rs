use scoop_identity::ConeIdentity;

use super::{ExternalShapeLinkImportV1, ShapeLinkContractV1, ShapeLinkError};
use crate::*;

pub(super) mod contracts;
mod support;
pub use support::ShapeLinkSupportV1;

pub struct ShapeLinkProviderPartsV1<'a> {
    pub foundation: &'a ConeLirFoundation,
    pub definitions: &'a ObjectSymbolSurfaceV1,
    pub support: ShapeLinkSupportV1<'a>,
    pub ordinary: &'a CrossConeLirBridgeSectionV1,
    pub layouts: &'a CanonicalExactLayoutExportsV1,
    pub callables: &'a CanonicalExactCallableAbiExportsV1,
    pub descriptors: &'a CanonicalExactDescriptorExportsV1,
    pub dispatch: &'a CanonicalExactDispatchExportsV1,
}

/// Borrowed provider data used to select an actual external definition.
pub struct ShapeLinkProviderV1<'a> {
    parts: ShapeLinkProviderPartsV1<'a>,
}

impl<'a> ShapeLinkProviderV1<'a> {
    pub fn try_new(parts: ShapeLinkProviderPartsV1<'a>) -> Result<Self, ShapeLinkError> {
        let provider = parts.foundation.producer();
        if [
            parts.ordinary.artifact(),
            parts.layouts.provider(),
            parts.callables.provider(),
            parts.descriptors.provider(),
            parts.dispatch.provider(),
            parts.support.providers()[0],
            parts.support.providers()[1],
        ]
        .into_iter()
        .any(|actual| actual != provider)
        {
            return Err(ShapeLinkError::Provider);
        }
        let target = parts.layouts.target();
        if parts.callables.target() != target
            || parts.descriptors.target() != target
            || parts.dispatch.target() != target
        {
            return Err(ShapeLinkError::Target);
        }
        Ok(Self { parts })
    }

    pub fn provider(&self) -> ConeIdentity {
        self.parts.foundation.producer()
    }

    pub fn target_profile(&self) -> LirTargetProfile {
        self.parts.layouts.target()
    }

    pub(crate) fn semantic_target(
        &self,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<Option<LayoutAbiSemanticTargetV1>, ShapeLinkError> {
        super::import::semantic_target(subject, self.parts.layouts, self.parts.callables)
    }

    /// The complete local Strong definition surface used by import replay.
    /// Closure validators use this exact surface for the consumer-side
    /// foreign-definition exclusion; it is not reconstructed from symbols.
    pub fn canonical_definitions(&self) -> &'a ObjectSymbolSurfaceV1 {
        self.parts.definitions
    }

    pub(super) fn import(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        consumer: ConeIdentity,
    ) -> Result<ExternalShapeLinkImportV1, ShapeLinkError> {
        if consumer == self.provider() {
            return Err(ShapeLinkError::LocalImport);
        }
        let physical = StrongShapeDefinitionRefV1::from_symbols(
            subject,
            self.parts.foundation,
            self.parts.definitions,
        )?;
        let contract = self.contract(subject)?;
        let import = ExternalShapeLinkImportV1 {
            provider: self.provider(),
            subject,
            expected_symbol: physical.symbol(),
            symbol: physical.symbol().symbol(),
            required_definition: physical.definition(),
            contract,
        };
        Ok(import)
    }
}
