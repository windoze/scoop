use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOwnerChain,
    LocalValueSelector, PackagePath, PersistentIdResolver, PersistentPropertyId,
    SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn local_place_uses_canonical_table_index_and_round_trips() {
    let expected = DefaultPlaceV1::Local {
        local: LocalValueSelector::Parameter {
            declaration_index: 0,
        },
    };
    let mut locals = LocalResolver::new(vec![
        LocalValueSelector::This,
        LocalValueSelector::Parameter {
            declaration_index: 0,
        },
    ]);
    let bytes = encode(&expected.index_local(&mut locals).unwrap()).unwrap();

    assert_eq!(hex(&bytes), "a200010101");
    let decoded: DecodedDefaultPlaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut PropertyResolver::empty(), &mut locals),
        Ok(expected)
    );
}

#[test]
fn global_place_uses_persistent_property_identity() {
    let property = property("state");
    let expected = DefaultPlaceV1::Global { property };
    let mut locals = LocalResolver::new(Vec::new());
    let bytes = encode(&expected.index_local(&mut locals).unwrap()).unwrap();
    let decoded: DecodedDefaultPlaceV1 = decode_canonical(&bytes).unwrap();

    assert_eq!(bytes[2], 2);
    assert_eq!(
        decoded.resolve(&mut PropertyResolver::new(property), &mut locals),
        Ok(expected)
    );
}

#[test]
fn place_indexing_and_resolution_report_the_failing_domain() {
    let local = LocalValueSelector::Parameter {
        declaration_index: 1,
    };
    let place = DefaultPlaceV1::Local {
        local: local.clone(),
    };
    let mut locals = LocalResolver::new(Vec::new());
    assert_eq!(
        place.index_local(&mut locals),
        Err(DefaultPlaceIndexError::Local(LocalError::MissingSelector(
            local
        )))
    );

    let decoded: DecodedDefaultPlaceV1 =
        decode_canonical(&encode(&IndexedDefaultPlaceV1::Local { local_index: 2 }).unwrap())
            .unwrap();
    assert_eq!(
        decoded.resolve(&mut PropertyResolver::empty(), &mut locals),
        Err(DefaultPlaceResolutionError::Local(
            LocalError::MissingIndex(2)
        ))
    );

    let missing = property("missing");
    let decoded: DecodedDefaultPlaceV1 =
        decode_canonical(&encode(&IndexedDefaultPlaceV1::Global { property: missing }).unwrap())
            .unwrap();
    assert_eq!(
        decoded.resolve(&mut PropertyResolver::empty(), &mut locals),
        Err(DefaultPlaceResolutionError::Global(PropertyError))
    );
}

#[test]
fn place_decoder_rejects_unknown_tags_and_non_exact_maps() {
    let error =
        decode_canonical::<DecodedDefaultPlaceV1>(&[0xa2, 0x00, 0x03, 0x01, 0x00]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error = decode_canonical::<DecodedDefaultPlaceV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

struct LocalResolver {
    selectors: Vec<LocalValueSelector>,
}

impl LocalResolver {
    const fn new(selectors: Vec<LocalValueSelector>) -> Self {
        Self { selectors }
    }
}

impl TemplateLocalSelectorResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_selector(
        &mut self,
        index: u32,
    ) -> Result<LocalValueSelector, Self::Error> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.selectors.get(index))
            .cloned()
            .ok_or(LocalError::MissingIndex(index))
    }
}

impl TemplateLocalIndexResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        self.selectors
            .iter()
            .position(|candidate| candidate == selector)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| LocalError::MissingSelector(selector.clone()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LocalError {
    MissingIndex(u32),
    MissingSelector(LocalValueSelector),
}

impl std::fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LocalError {}

struct PropertyResolver {
    property: Option<PersistentPropertyId>,
}

impl PropertyResolver {
    const fn new(property: PersistentPropertyId) -> Self {
        Self {
            property: Some(property),
        }
    }

    const fn empty() -> Self {
        Self { property: None }
    }
}

impl PersistentIdResolver<PersistentPropertyId> for PropertyResolver {
    type Error = PropertyError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentPropertyId>,
    ) -> Result<PersistentPropertyId, Self::Error> {
        self.property
            .filter(|property| property.as_array() == id.as_array())
            .ok_or(PropertyError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PropertyError;

impl std::fmt::Display for PropertyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("unknown property")
    }
}

impl std::error::Error for PropertyError {}

fn property(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
