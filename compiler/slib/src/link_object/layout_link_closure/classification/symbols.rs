use scoop_identity::PersistentSymbolRequest;
use scoop_lir::{CanonicalExternalShapeLinkImportsV1, LirTargetProfile};
use scoop_wire::WirePath;

use super::LayoutLinkClosureError;
use crate::link_object::VerifiedCrossConeStrongRequirementClosureV1;

pub(super) struct ImportSymbolIndex {
    symbols: Vec<(Vec<u8>, u32)>,
}

impl ImportSymbolIndex {
    pub(super) fn new(
        imports: &CanonicalExternalShapeLinkImportsV1,
        target: LirTargetProfile,
    ) -> Result<Self, LayoutLinkClosureError> {
        Self::from_requests(
            imports
                .records()
                .iter()
                .map(|import| import.expected_symbol()),
            target,
        )
    }

    fn from_requests(
        requests: impl ExactSizeIterator<Item = PersistentSymbolRequest>,
        target: LirTargetProfile,
    ) -> Result<Self, LayoutLinkClosureError> {
        let path = WirePath::root();
        let count = requests.len();
        u32::try_from(count).map_err(|_| LayoutLinkClosureError::ImportIndexOverflow)?;

        let mut symbols = Vec::new();
        scoop_wire::allocation::try_reserve(&mut symbols, count, &path)?;

        for (index, request) in requests.enumerate() {
            let symbol = normalized(request, target)?;

            symbols.push((symbol, index as u32));
        }
        symbols.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        for pair in symbols.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(LayoutLinkClosureError::DuplicateNormalizedSymbol {
                    first: pair[0].1,
                    second: pair[1].1,
                });
            }
        }
        Ok(Self { symbols })
    }

    pub(super) fn len(&self) -> usize {
        self.symbols.len()
    }

    pub(super) fn find(&self, symbol: &[u8]) -> Result<Option<u32>, LayoutLinkClosureError> {
        Ok(self
            .symbols
            .binary_search_by(|entry| entry.0.as_slice().cmp(symbol))
            .ok()
            .map(|index| self.symbols[index].1))
    }

    pub(super) fn reject_old_partitions(
        &self,
        legacy: &VerifiedCrossConeStrongRequirementClosureV1,
    ) -> Result<(), LayoutLinkClosureError> {
        for import in legacy.semantic_imports().imports() {
            let symbol = normalized(import.expected_symbol(), legacy.target())?;
            if let Some(import_index) = self.find(&symbol)? {
                return Err(LayoutLinkClosureError::OldPartition { import_index });
            }
        }
        for requirement in legacy.external_requirements() {
            if let Some(import_index) = self.find(requirement.use_site().symbol())? {
                return Err(LayoutLinkClosureError::OldPartition { import_index });
            }
        }
        Ok(())
    }
}

fn normalized(
    request: PersistentSymbolRequest,
    target: LirTargetProfile,
) -> Result<Vec<u8>, LayoutLinkClosureError> {
    let normalization = target.contract().native_symbol_normalization();

    Ok(normalization
        .compiler_generated_object_symbol(request.symbol().as_str())
        .into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link_object::layout_link_closure::tests::fixture::{Provider, TARGET};
    use scoop_identity::MangledSymbol;

    #[test]
    fn layout_link_normalized_imports_reject_duplicates() {
        let provider = Provider::new();
        let request = provider
            .import(scoop_identity::ConeIdentity::SINGLE_FILE)
            .expected_symbol();
        assert!(matches!(
            ImportSymbolIndex::from_requests([request, request].into_iter(), TARGET),
            Err(LayoutLinkClosureError::DuplicateNormalizedSymbol { .. })
        ));

        let symbols = ImportSymbolIndex::from_requests([request].into_iter(), TARGET).unwrap();
        assert_eq!(
            symbols.symbols[0].0.len(),
            MangledSymbol::byte_length_for_key(&request.key()) + 1
        );
        assert_eq!(symbols.find(&symbols.symbols[0].0).unwrap(), Some(0));
    }

    #[test]
    fn layout_link_rejects_symbols_already_owned_by_the_ordinary_callable_partition() {
        use crate::link_object::native_requirements::tests::dependency_closure;
        use crate::link_object::strong_relocation_closure::tests::verified_member_with_undefined;
        use crate::link_object::symbol_verification::tests::fixture_for_producer;
        use crate::link_object::verify_cross_cone_strong_requirements_v1;
        use scoop_identity::*;
        use scoop_lir::*;

        let provider = ConeCoordinate::new("test", "old-callable", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    provider,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new("entry").unwrap(),
                0,
                None,
                vec![],
            ))
            .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let abi = CanonicalScoopAbiFunctionSignature::new(
            ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit),
            vec![],
            ScoopAbiReturn::unit_void(),
            scoop_identity::GcEffect::NoGc,
        )
        .unwrap();
        let selected = SelectedDependencyLirCallableV1::new(
            provider,
            DependencyCallableDeclarationId::Function(function),
            StrongCallableDefinitionOwner::Function(function),
            abi,
            scoop_lir::CallingConvention::Cdecl,
            ExternalCallableRootPlan::NoGc,
        )
        .unwrap();
        let consumer = ConeIdentity::SINGLE_FILE;
        let foundation =
            OdrFreeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
        let bridge =
            CrossConeLirBridgeSectionV1::try_new(&foundation, vec![], vec![selected]).unwrap();
        let imports = crate::CrossConeLinkSemanticImportSetV1::from_lir_bridge(&bridge).unwrap();
        let request = imports.imports()[0].expected_symbol();
        let name = normalized(request, TARGET).unwrap();
        let object = fixture_for_producer(consumer, "oldCall");
        let core = dependency_closure(consumer, verified_member_with_undefined(&object, &name));
        let provider_object = fixture_for_producer(provider, "entry");
        let provider_strong = crate::verify_current_cone_strong_relocation_closure_v1(vec![
            crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations(&provider_object),
        ]).unwrap();
        let owners = crate::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
            &provider_strong,
        )
        .unwrap();
        let legacy = verify_cross_cone_strong_requirements_v1(
            core.target(),
            core.strong_closure().clone(),
            core.external_bridges().clone(),
            &[owners],
            &bridge,
        )
        .unwrap();
        assert_eq!(legacy.requirements().len(), 1);
        let attempted = ImportSymbolIndex::from_requests([request].into_iter(), TARGET).unwrap();
        assert!(matches!(
            attempted.reject_old_partitions(&legacy),
            Err(LayoutLinkClosureError::OldPartition { import_index: 0 })
        ));
    }
}
