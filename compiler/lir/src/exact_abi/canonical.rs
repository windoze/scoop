use scoop_identity::{
    CanonicalScoopStorage, ScoopAbiArgument, ScoopAbiError, ScoopAbiReturn, ScoopAbiValueShape,
};

use crate::{LirTargetProfile, ScoopAbiPassing};

/// Classifies one logical argument with the target's common Scoop ABI rule.
pub fn canonical_scoop_abi_argument(
    target: LirTargetProfile,
    storage: CanonicalScoopStorage,
) -> Result<ScoopAbiArgument, ScoopAbiError> {
    if storage.byte_size() == 0 {
        ScoopAbiArgument::elided_zst(storage)
    } else {
        match passing(target, storage.shape()) {
            ScoopAbiPassing::Direct => ScoopAbiArgument::direct(storage),
            ScoopAbiPassing::Indirect => ScoopAbiArgument::indirect(storage),
        }
    }
}

/// Unit is handled by the source/layout role; every value return uses this rule.
pub fn canonical_scoop_abi_value_return(
    target: LirTargetProfile,
    storage: CanonicalScoopStorage,
) -> Result<ScoopAbiReturn, ScoopAbiError> {
    if storage.byte_size() == 0 {
        ScoopAbiReturn::elided_zst(storage)
    } else {
        match passing(target, storage.shape()) {
            ScoopAbiPassing::Direct => ScoopAbiReturn::direct(storage),
            ScoopAbiPassing::Indirect => ScoopAbiReturn::indirect(storage),
        }
    }
}

fn passing(target: LirTargetProfile, shape: ScoopAbiValueShape) -> ScoopAbiPassing {
    target.classify_scoop_abi_value(match shape {
        ScoopAbiValueShape::Scalar => crate::ScoopAbiValueShape::Scalar,
        ScoopAbiValueShape::Aggregate => crate::ScoopAbiValueShape::Aggregate,
    })
}
