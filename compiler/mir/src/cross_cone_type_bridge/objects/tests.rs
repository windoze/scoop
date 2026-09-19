use super::*;
use scoop_identity::{
    AccessorRole, CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerAtom,
    DefinitionOwnerChain, Effect, ExactCallableSignature, PackagePath, PendingIdentityValidation,
    PropertyAccessorKey, PropertyOwner, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod initialization;
mod records;
pub(in crate::cross_cone_type_bridge) mod support;
mod wire;
use support::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
