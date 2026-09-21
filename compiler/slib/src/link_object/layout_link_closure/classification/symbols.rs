use scoop_identity::{MangledSymbol, PersistentSymbolRequest};
use scoop_lir::{CanonicalExternalShapeLinkImportsV1, LirTargetProfile, NativeSymbolNormalization};
use scoop_wire::{BudgetMeter, WirePath};

use super::LayoutLinkClosureError;
use crate::link_object::VerifiedCrossConeStrongRequirementClosureV1;

pub(super) struct ImportSymbolIndex {
    symbols: Vec<(Vec<u8>, u32)>,
}

impl ImportSymbolIndex {
    pub(super) fn new(
        imports: &CanonicalExternalShapeLinkImportsV1<'_>,
        target: LirTargetProfile,
        meter: &mut BudgetMeter,
    ) -> Result<Self, LayoutLinkClosureError> {
        Self::from_requests(
            imports
                .records()
                .iter()
                .map(|import| import.expected_symbol()),
            target,
            meter,
        )
    }

    fn from_requests(
        requests: impl ExactSizeIterator<Item = PersistentSymbolRequest>,
        target: LirTargetProfile,
        meter: &mut BudgetMeter,
    ) -> Result<Self, LayoutLinkClosureError> {
        let path = WirePath::root();
        let count = requests.len();
        u32::try_from(count).map_err(|_| LayoutLinkClosureError::ImportIndexOverflow)?;
        meter.check_table_entries(count as u64, &path)?;
        meter.charge_nodes(count as u64, &path)?;
        let mut symbols = Vec::new();
        meter.try_reserve_collection_slots(&mut symbols, count, &path)?;
        let depth = u64::from(count.max(1).ilog2()) + 1;
        for (index, request) in requests.enumerate() {
            let symbol = normalized(request, target, meter)?;
            meter.charge_work((symbol.len() as u64 + 1).saturating_mul(depth), &path)?;
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

    pub(super) fn find(
        &self,
        symbol: &[u8],
        meter: &mut BudgetMeter,
    ) -> Result<Option<u32>, LayoutLinkClosureError> {
        let depth = u64::from(self.symbols.len().max(1).ilog2()) + 1;
        meter.charge_work(
            (symbol.len() as u64 + 1).saturating_mul(depth),
            &WirePath::root(),
        )?;
        Ok(self
            .symbols
            .binary_search_by(|entry| entry.0.as_slice().cmp(symbol))
            .ok()
            .map(|index| self.symbols[index].1))
    }

    pub(super) fn reject_old_partitions(
        &self,
        legacy: &VerifiedCrossConeStrongRequirementClosureV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), LayoutLinkClosureError> {
        for import in legacy.semantic_imports().imports() {
            let symbol = normalized(import.expected_symbol(), legacy.target(), meter)?;
            if let Some(import_index) = self.find(&symbol, meter)? {
                return Err(LayoutLinkClosureError::OldPartition { import_index });
            }
        }
        for requirement in legacy.core_closure().core_requirements() {
            if let Some(import_index) = self.find(requirement.use_site().symbol(), meter)? {
                return Err(LayoutLinkClosureError::OldPartition { import_index });
            }
        }
        Ok(())
    }
}

fn normalized(
    request: PersistentSymbolRequest,
    target: LirTargetProfile,
    meter: &mut BudgetMeter,
) -> Result<Vec<u8>, LayoutLinkClosureError> {
    let path = WirePath::root();
    let logical = MangledSymbol::byte_length_for_key(&request.key()) as u64;
    let normalization = target.contract().native_symbol_normalization();
    let physical = match normalization {
        NativeSymbolNormalization::MachOExternalUnderscore => logical + 1,
    };
    meter.charge_owned_bytes(logical + physical, &path)?;
    meter.charge_heap(logical + physical, &path)?;
    meter.charge_work(logical + physical, &path)?;
    Ok(normalization
        .compiler_generated_object_symbol(request.symbol().as_str())
        .into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link_object::layout_link_closure::tests::fixture::{Provider, TARGET, meter};

    #[test]
    fn layout_link_duplicate_normalized_imports_and_preallocation_budget_are_rejected() {
        let provider = Provider::new();
        let request = provider
            .import(scoop_identity::ConeIdentity::SINGLE_FILE)
            .expected_symbol();
        assert!(matches!(
            ImportSymbolIndex::from_requests([request, request].into_iter(), TARGET, &mut meter()),
            Err(LayoutLinkClosureError::DuplicateNormalizedSymbol { .. })
        ));
        let mut limited = BudgetMeter::new(scoop_wire::DecodeLimits {
            owned_bytes: 0,
            ..Default::default()
        });
        assert!(matches!(
            ImportSymbolIndex::from_requests([request].into_iter(), TARGET, &mut limited),
            Err(LayoutLinkClosureError::Resource(_))
        ));
        let symbols =
            ImportSymbolIndex::from_requests([request].into_iter(), TARGET, &mut meter()).unwrap();
        assert_eq!(
            symbols.symbols[0].0.len(),
            MangledSymbol::byte_length_for_key(&request.key()) + 1
        );
        assert_eq!(
            symbols.find(&symbols.symbols[0].0, &mut meter()).unwrap(),
            Some(0)
        );
    }

    #[test]
    fn layout_link_rejects_symbols_already_owned_by_the_ordinary_callable_partition() {
        use crate::link_object::native_requirements::tests::core_closure;
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
        let name = normalized(request, TARGET, &mut meter()).unwrap();
        let object = fixture_for_producer(consumer, "oldCall");
        let core = core_closure(consumer, verified_member_with_undefined(&object, &name));
        let legacy = verify_cross_cone_strong_requirements_v1(core, &bridge).unwrap();
        assert_eq!(legacy.requirements().len(), 1);
        let attempted =
            ImportSymbolIndex::from_requests([request].into_iter(), TARGET, &mut meter()).unwrap();
        assert!(matches!(
            attempted.reject_old_partitions(&legacy, &mut meter()),
            Err(LayoutLinkClosureError::OldPartition { import_index: 0 })
        ));
    }
}
