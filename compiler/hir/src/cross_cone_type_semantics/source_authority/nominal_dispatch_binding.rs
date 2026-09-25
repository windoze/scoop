//! Joins independent dispatch evidence to complete nominal source contracts.
use super::nominal_nested_binding::compare;
use crate::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity, ExactTypeKey, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};
use std::collections::{BTreeMap, BTreeSet};

mod callables;
mod errors;
mod inventory;
mod members;
mod relations;
mod source_adapter;
pub use errors::*;
type Error = NominalDispatchBindingError;

/// Agreement of two source transcripts. Candidate slot replay and machine-use
/// validation remain obligations of the enclosing section transaction.
#[derive(Debug)]
pub struct BoundNominalDispatchSourcesV1<'d, 'p, 's, 'a, 'f> {
    parameters: &'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>,
    slots: &'d BoundInheritanceSlotSourcesV1<'s, 'a, 'f>,
    required: CanonicalProtectedDeclarationRefsV1,
}
impl<'d, 'p, 's, 'a, 'f> BoundNominalDispatchSourcesV1<'d, 'p, 's, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.parameters.provider()
    }
    pub const fn parameters(&self) -> &'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
        self.parameters
    }
    pub const fn slots(&self) -> &'d BoundInheritanceSlotSourcesV1<'s, 'a, 'f> {
        self.slots
    }
    pub fn inventory(&self) -> &'a CanonicalSourceInheritanceInventoriesV1 {
        self.slots.dispatch.inventory()
    }
}
impl<'p, 's, 'a, 'f> BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
    pub fn bind_dispatch_sources<'d>(
        &'d self,
        slots: &'d BoundInheritanceSlotSourcesV1<'s, 'a, 'f>,
    ) -> Result<BoundNominalDispatchSourcesV1<'d, 'p, 's, 'a, 'f>, Error> {
        let foundation = self.members().nominals.foundation;
        if !std::ptr::eq(foundation, slots.dispatch.foundation) {
            return Err(Error::FoundationMismatch);
        }
        let unit = slots.unit_exact_type().map_err(Error::from_slot)?;
        if foundation
            .exact_type_key(unit)
            .map_err(NominalNestedBindingError::from)?
            != &ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            )
        {
            return Err(Error::UnitType);
        }
        let required = super::protected_declaration_binding::required_declarations(self)?;
        let bound = BoundNominalDispatchSourcesV1 {
            parameters: self,
            slots,
            required,
        };
        inventory::validate(&bound)?;
        callables::validate(&bound)?;
        relations::validate(&bound)?;
        Ok(bound)
    }
}
fn origin(declaration: InheritanceCallableDeclarationV1) -> CallableTemplateOrigin {
    match declaration {
        InheritanceCallableDeclarationV1::Function(id) => CallableTemplateOrigin::Function(id),
        InheritanceCallableDeclarationV1::Getter(id)
        | InheritanceCallableDeclarationV1::Setter(id) => CallableTemplateOrigin::Accessor(id),
    }
}
