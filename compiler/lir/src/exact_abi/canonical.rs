use scoop_identity::{
    CanonicalScoopStorage, ScoopAbiArgument, ScoopAbiError, ScoopAbiReturn, ScoopAbiValueShape,
};

use crate::{
    ExactRepresentationKindV1, ExactValueLayoutV1, IntrinsicValueFamilyV1, LirTargetProfile,
    ScoopAbiPassing,
};

impl ExactValueLayoutV1 {
    pub fn scoop_abi_argument(
        &self,
        target: LirTargetProfile,
    ) -> Result<ScoopAbiArgument, ScoopAbiError> {
        canonical_scoop_abi_argument(target, self.scoop_storage())
    }

    pub fn scoop_abi_return(
        &self,
        target: LirTargetProfile,
    ) -> Result<ScoopAbiReturn, ScoopAbiError> {
        if matches!(
            self.representation().kind(),
            ExactRepresentationKindV1::IntrinsicValue(IntrinsicValueFamilyV1::Unit)
        ) {
            Ok(ScoopAbiReturn::unit_void())
        } else {
            canonical_scoop_abi_value_return(target, self.scoop_storage())
        }
    }

    fn scoop_storage(&self) -> CanonicalScoopStorage {
        let shape = match self.representation().kind() {
            ExactRepresentationKindV1::Scalar(_)
            | ExactRepresentationKindV1::QualifiedPointer(_)
            | ExactRepresentationKindV1::NicheEnum(_) => ScoopAbiValueShape::Scalar,
            ExactRepresentationKindV1::Struct(_)
            | ExactRepresentationKindV1::Tuple(_)
            | ExactRepresentationKindV1::TaggedEnum(_)
            | ExactRepresentationKindV1::IntrinsicValue(_) => ScoopAbiValueShape::Aggregate,
        };
        let storage = self.value().storage();
        CanonicalScoopStorage::new(
            self.identity().exact(),
            storage.byte_size(),
            storage.alignment().as_nonzero(),
            shape,
        )
    }
}

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
