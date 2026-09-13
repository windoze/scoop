/// Projects the sealed HIR output branch into the MIR entry bridge without
/// dropping any part of the executable source-entry proof.
pub fn lower_entry_bridge(
    output: &scoop_hir::HirOutputContractV1,
) -> Result<scoop_mir::EntryMirBridgeBranchV1, scoop_mir::MirProductionBuildError> {
    match output {
        scoop_hir::HirOutputContractV1::Library => Ok(scoop_mir::EntryMirBridgeBranchV1::Library),
        scoop_hir::HirOutputContractV1::Executable(source) => {
            let implementation = scoop_mir::CallableOwner::Function(source.declaration());
            scoop_mir::EntryMirBridgeV1::new(source.as_ref().clone(), implementation)
                .map(Box::new)
                .map(scoop_mir::EntryMirBridgeBranchV1::Executable)
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_hir::HirOutputContractV1;
    use scoop_identity::{
        CallableOwner, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
        ExecutableSourceEntryIdentity, PackagePath, PersistentExactTypeId, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    use super::lower_entry_bridge;

    #[test]
    fn lowering_preserves_the_complete_executable_source_proof() {
        let source = source();
        let output = HirOutputContractV1::Executable(Box::new(source.clone()));
        let bridge = lower_entry_bridge(&output).unwrap();
        let scoop_mir::EntryMirBridgeBranchV1::Executable(bridge) = bridge else {
            panic!("the executable HIR branch has an executable MIR branch")
        };

        assert_eq!(bridge.source(), &source);
        assert_eq!(
            bridge.implementation(),
            CallableOwner::Function(source.declaration())
        );
        assert_eq!(
            lower_entry_bridge(&HirOutputContractV1::Library).unwrap(),
            scoop_mir::EntryMirBridgeBranchV1::Library
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
