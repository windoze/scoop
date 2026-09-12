use crate::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, IntegerBitWidth,
    PackagePath, PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

/// Trusted core nominal roles with special native-boundary storage semantics.
///
/// Consumers identify a role by its typed persistent declaration identity;
/// names from an artifact are never used as a fallback.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoreNativeBoundaryNominal {
    Unit,
    Boolean,
    Signed8,
    Signed16,
    Signed32,
    Signed64,
    Unsigned8,
    Unsigned16,
    Unsigned32,
    Unsigned64,
    Option,
    Ptr,
    FunPtr,
    PinnedPtr,
    GcHandle,
}

impl CoreNativeBoundaryNominal {
    pub const ALL: [Self; 15] = [
        Self::Unit,
        Self::Boolean,
        Self::Signed8,
        Self::Signed16,
        Self::Signed32,
        Self::Signed64,
        Self::Unsigned8,
        Self::Unsigned16,
        Self::Unsigned32,
        Self::Unsigned64,
        Self::Option,
        Self::Ptr,
        Self::FunPtr,
        Self::PinnedPtr,
        Self::GcHandle,
    ];

    pub fn declaration_key(self) -> SourceDeclarationKey {
        let (name, kind, type_parameter_count) = match self {
            Self::Unit => ("Unit", SourceNominalKind::Struct, 0),
            Self::Boolean => ("Boolean", SourceNominalKind::Struct, 0),
            Self::Signed8 => ("Int8", SourceNominalKind::Struct, 0),
            Self::Signed16 => ("Int16", SourceNominalKind::Struct, 0),
            Self::Signed32 => ("Int", SourceNominalKind::Struct, 0),
            Self::Signed64 => ("Long", SourceNominalKind::Struct, 0),
            Self::Unsigned8 => ("UInt8", SourceNominalKind::Struct, 0),
            Self::Unsigned16 => ("UInt16", SourceNominalKind::Struct, 0),
            Self::Unsigned32 => ("UInt", SourceNominalKind::Struct, 0),
            Self::Unsigned64 => ("ULong", SourceNominalKind::Struct, 0),
            Self::Option => ("Option", SourceNominalKind::Enum, 1),
            Self::Ptr => ("Ptr", SourceNominalKind::Struct, 1),
            Self::FunPtr => ("FunPtr", SourceNominalKind::Struct, 1),
            Self::PinnedPtr => ("PinnedPtr", SourceNominalKind::Struct, 1),
            Self::GcHandle => ("GcHandle", SourceNominalKind::Struct, 1),
        };
        SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .expect("trusted core declaration site is valid"),
            CanonicalIdentifier::new(name).expect("trusted core name is canonical"),
            kind,
            type_parameter_count,
        )
    }

    pub fn concrete_id(self) -> Option<PersistentTypeId> {
        match self {
            Self::Unit
            | Self::Boolean
            | Self::Signed8
            | Self::Signed16
            | Self::Signed32
            | Self::Signed64
            | Self::Unsigned8
            | Self::Unsigned16
            | Self::Unsigned32
            | Self::Unsigned64 => Some(
                PersistentTypeId::from_source_declaration(&self.declaration_key())
                    .expect("trusted concrete core nominal has a concrete identity"),
            ),
            Self::Option | Self::Ptr | Self::FunPtr | Self::PinnedPtr | Self::GcHandle => None,
        }
    }

    pub fn generic_id(self) -> Option<PersistentGenericTypeId> {
        match self {
            Self::Option | Self::Ptr | Self::FunPtr | Self::PinnedPtr | Self::GcHandle => Some(
                PersistentGenericTypeId::from_source_declaration(&self.declaration_key())
                    .expect("trusted generic core nominal has a generic identity"),
            ),
            Self::Unit
            | Self::Boolean
            | Self::Signed8
            | Self::Signed16
            | Self::Signed32
            | Self::Signed64
            | Self::Unsigned8
            | Self::Unsigned16
            | Self::Unsigned32
            | Self::Unsigned64 => None,
        }
    }

    pub const fn integer(self) -> Option<(crate::Signedness, IntegerBitWidth)> {
        use crate::Signedness::{Signed, Unsigned};
        match self {
            Self::Signed8 => Some((Signed, IntegerBitWidth::Bits8)),
            Self::Signed16 => Some((Signed, IntegerBitWidth::Bits16)),
            Self::Signed32 => Some((Signed, IntegerBitWidth::Bits32)),
            Self::Signed64 => Some((Signed, IntegerBitWidth::Bits64)),
            Self::Unsigned8 => Some((Unsigned, IntegerBitWidth::Bits8)),
            Self::Unsigned16 => Some((Unsigned, IntegerBitWidth::Bits16)),
            Self::Unsigned32 => Some((Unsigned, IntegerBitWidth::Bits32)),
            Self::Unsigned64 => Some((Unsigned, IntegerBitWidth::Bits64)),
            Self::Unit
            | Self::Boolean
            | Self::Option
            | Self::Ptr
            | Self::FunPtr
            | Self::PinnedPtr
            | Self::GcHandle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CoreNativeBoundaryNominal;

    #[test]
    fn native_core_roles_have_distinct_typed_declaration_ids() {
        let concrete = CoreNativeBoundaryNominal::ALL
            .into_iter()
            .filter_map(CoreNativeBoundaryNominal::concrete_id)
            .collect::<std::collections::BTreeSet<_>>();
        let generic = CoreNativeBoundaryNominal::ALL
            .into_iter()
            .filter_map(CoreNativeBoundaryNominal::generic_id)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(concrete.len(), 10);
        assert_eq!(generic.len(), 5);
    }

    #[test]
    fn unit_role_reuses_the_sealed_core_builtin_identity() {
        assert_eq!(
            CoreNativeBoundaryNominal::Unit.concrete_id(),
            Some(crate::CoreBuiltinNominal::Unit.identity_record().id())
        );
    }
}
