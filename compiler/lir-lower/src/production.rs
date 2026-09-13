/// Projects the validated MIR entry branch into the LIR production source.
/// The complete HIR proof remains intact through this stage boundary.
pub fn lower_entry_production_source(
    entry: &scoop_mir::EntryMirBridgeBranchV1,
) -> scoop_lir::EntryProductionSourceV1 {
    match entry {
        scoop_mir::EntryMirBridgeBranchV1::Library => scoop_lir::EntryProductionSourceV1::Library,
        scoop_mir::EntryMirBridgeBranchV1::Executable(bridge) => {
            scoop_lir::EntryProductionSourceV1::executable(bridge.source().clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableOwner, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
        ExecutableSourceEntryIdentity, PackagePath, PersistentExactTypeId, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    use super::lower_entry_production_source;

    #[test]
    fn lowering_preserves_the_complete_mir_entry_proof() {
        let source = source();
        let implementation = CallableOwner::Function(source.declaration());
        let mir = scoop_mir::EntryMirBridgeBranchV1::Executable(Box::new(
            scoop_mir::EntryMirBridgeV1::new(source.clone(), implementation).unwrap(),
        ));

        assert_eq!(
            lower_entry_production_source(&mir),
            scoop_lir::EntryProductionSourceV1::executable(source)
        );
        assert_eq!(
            lower_entry_production_source(&scoop_mir::EntryMirBridgeBranchV1::Library),
            scoop_lir::EntryProductionSourceV1::Library
        );
    }

    fn source() -> ExecutableSourceEntryIdentity {
        let declaration = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("main").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        ExecutableSourceEntryIdentity::try_new(
            &declaration,
            ExactOrdinaryNoArgUnitSignature::new(unit),
        )
        .unwrap()
    }
}
