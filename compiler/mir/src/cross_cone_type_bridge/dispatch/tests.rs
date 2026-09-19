use super::*;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerAtom,
    DefinitionOwnerChain, DispatchSlotKey, Effect, ExactCallableSignature, GeneratedCallableKey,
    PackagePath, PendingIdentityValidation, PersistentFunctionId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod dependencies;
mod records;
mod rejections;
pub(in crate::cross_cone_type_bridge) mod support;
mod wire;
use support::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
