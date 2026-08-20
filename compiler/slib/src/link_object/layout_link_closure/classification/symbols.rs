use scoop_identity::PersistentSymbolRequest;
use scoop_lir::{CanonicalExternalShapeLinkImportsV1, LirTargetProfile};
use scoop_wire::WirePath;

use super::LayoutLinkClosureError;

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

    pub(super) fn find(&self, symbol: &[u8]) -> Result<Option<u32>, LayoutLinkClosureError> {
        Ok(self
            .symbols
            .binary_search_by(|entry| entry.0.as_slice().cmp(symbol))
            .ok()
            .map(|index| self.symbols[index].1))
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
}
