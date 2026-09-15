use crate::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    PackagePath, PersistentExportBindingId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

/// Closed origin of a callable that an ordinary Cone may import from trusted
/// core during the currently supported bridge phase.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoreImportedCallableKind {
    Prelude(PersistentExportBindingId),
    InitializationCycleThrower,
}

/// Compiler-owned nominal types that have no ordinary source declaration
/// arena entry. Their source-shaped keys are fixed by trusted core authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoreBuiltinNominal {
    Unit,
    Any,
}

impl CoreBuiltinNominal {
    pub fn declaration_key(self) -> SourceDeclarationKey {
        let site = SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .expect("a cone-wide trusted core declaration site is always valid");
        let (name, kind) = match self {
            Self::Unit => ("Unit", SourceNominalKind::Struct),
            Self::Any => ("Any", SourceNominalKind::Class),
        };
        SourceDeclarationKey::nominal(
            site,
            CanonicalIdentifier::new(name)
                .expect("trusted core builtin names are canonical identifiers"),
            kind,
            0,
        )
    }

    pub fn identity_record(self) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
        CborIdentityRecord::from_key(self.declaration_key())
            .expect("trusted core builtin declaration keys are concrete nominals")
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::CoreBuiltinNominal;
    use crate::{
        DeclarationName, DeclarationScope, DuplicateSignatureKey, PackagePath,
        SourceDeclarationKind,
    };

    #[test]
    fn builtin_declaration_keys_are_sealed_to_trusted_core() {
        let unit = CoreBuiltinNominal::Unit.declaration_key();
        assert_eq!(unit.origin(), crate::ConeIdentity::CORE);
        assert_eq!(unit.package(), &PackagePath::root());
        assert!(unit.owners().owners().is_empty());
        assert_eq!(unit.scope(), &DeclarationScope::ConeWide);
        assert_eq!(
            unit.name(),
            &DeclarationName::Named(crate::CanonicalIdentifier::new("Unit").unwrap())
        );
        assert_eq!(unit.declaration_kind(), SourceDeclarationKind::Struct);
        assert_eq!(
            unit.duplicate_signature(),
            &DuplicateSignatureKey::Nominal {
                type_parameter_count: 0
            }
        );

        let any = CoreBuiltinNominal::Any.declaration_key();
        assert_eq!(any.origin(), crate::ConeIdentity::CORE);
        assert_eq!(any.package(), &PackagePath::root());
        assert!(any.owners().owners().is_empty());
        assert_eq!(any.scope(), &DeclarationScope::ConeWide);
        assert_eq!(
            any.name(),
            &DeclarationName::Named(crate::CanonicalIdentifier::new("Any").unwrap())
        );
        assert_eq!(any.declaration_kind(), SourceDeclarationKind::Class);
    }

    #[test]
    fn builtin_nominals_have_fixed_distinct_records() {
        let unit = CoreBuiltinNominal::Unit.identity_record();
        let any = CoreBuiltinNominal::Any.identity_record();

        assert_eq!(
            unit.id().to_string(),
            "ea1de3597e0f30acca2c62c6d871e693648ef7e1097cc8b0082d316acd7e7639"
        );
        assert_eq!(
            hex(&encode(&unit).unwrap()),
            "a2015820ea1de3597e0f30acca2c62c6d871e693648ef7e1097cc8b0082d316acd7e763902a70158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d0280038004a200010164556e6974050306a20001010007a10001"
        );
        assert_eq!(
            any.id().to_string(),
            "1d240c1f2259ce18ebc40ee2496cddf7f351ac358928802cd1bdc6347cc64a61"
        );
        assert_eq!(
            hex(&encode(&any).unwrap()),
            "a20158201d240c1f2259ce18ebc40ee2496cddf7f351ac358928802cd1bdc6347cc64a6102a70158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d0280038004a200010163416e79050106a20001010007a10001"
        );
        assert_ne!(unit.id(), any.id());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
