use scoop_identity::{CanonicalScoopStorage, ScoopAbiValueShape};

use crate::{ExactRepresentationKindV1, ExactValueLayoutV1};

impl ExactValueLayoutV1 {
    pub fn canonical_storage(&self) -> CanonicalScoopStorage {
        let shape = match self.representation().kind() {
            ExactRepresentationKindV1::Interface => ScoopAbiValueShape::Interface,
            ExactRepresentationKindV1::Scalar(_)
            | ExactRepresentationKindV1::QualifiedPointer(_) => ScoopAbiValueShape::Scalar,
            ExactRepresentationKindV1::NicheEnum(value) => match value.pointer_kind() {
                crate::NullNicheKind::Interface => ScoopAbiValueShape::Interface,
                _ => ScoopAbiValueShape::Scalar,
            },
            ExactRepresentationKindV1::Struct(_)
            | ExactRepresentationKindV1::MaybeUninit(_)
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
