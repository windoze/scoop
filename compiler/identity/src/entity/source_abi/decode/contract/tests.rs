use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedSourceNativeExternalContract, DecodedSourceNativeExternalContractKey,
    DecodedSourceNativeExternalContractRecord, DecodedSourceNativeExternalOwner,
    SourceNativeExternalResolutionError,
};
use crate::{
    CanonicalIdentifier, CanonicalNativeLibraryName, CanonicalNativeNameError, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, PersistentFunctionId, PersistentGenericTypeId,
    PersistentIdMismatch, PersistentIdResolver, PersistentKeyResolver, PersistentPropertyId,
    PersistentTypeId, SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn,
    SourceCallingConvention, SourceDeclarationKey, SourceDeclarationSite, SourceExternFunctionAbi,
    SourceNativeContractError, SourceNativeExternalContract, SourceNativeExternalContractKey,
    SourceNativeExternalContractRecord, SourceNativeExternalOwner, SourceNativeLibraryBinding,
    SourceNativeSymbol, SourceNativeSymbolError, SourceScoopAbiFunctionSignature,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver {
    function: SourceDeclarationKey,
    property: SourceDeclarationKey,
}

impl PersistentKeyResolver<PersistentFunctionId, SourceDeclarationKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: crate::DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<std::sync::Arc<SourceDeclarationKey>, Self::Error> {
        let expected = PersistentFunctionId::from_source_declaration(&self.function)
            .map_err(|_| ResolutionError)?;
        id.verify(expected)
            .map_err(|_: PersistentIdMismatch<PersistentFunctionId>| ResolutionError)?;
        Ok(std::sync::Arc::new(self.function.clone()))
    }
}

impl PersistentKeyResolver<PersistentPropertyId, SourceDeclarationKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: crate::DecodedPersistentId<PersistentPropertyId>,
    ) -> Result<std::sync::Arc<SourceDeclarationKey>, Self::Error> {
        let expected = PersistentPropertyId::from_source_declaration(&self.property)
            .map_err(|_| ResolutionError)?;
        id.verify(expected)
            .map_err(|_: PersistentIdMismatch<PersistentPropertyId>| ResolutionError)?;
        Ok(std::sync::Arc::new(self.property.clone()))
    }
}

impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        id.verify(plain_type())
            .map_err(|_: PersistentIdMismatch<PersistentTypeId>| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        id.verify(generic_type())
            .map_err(|_: PersistentIdMismatch<PersistentGenericTypeId>| ResolutionError)
    }
}

#[test]
fn source_contract_keys_round_trip_and_rebuild_from_declarations() {
    let function = function_declaration(None);
    let property = property_declaration();
    let keys = [
        SourceNativeExternalContractKey::function(&function).unwrap(),
        SourceNativeExternalContractKey::property(&property).unwrap(),
    ];
    let mut resolver = Resolver { function, property };

    for key in keys {
        let decoded = decode_canonical::<DecodedSourceNativeExternalContractKey>(
            &encode(&key).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut resolver).unwrap(), key);
    }
}

#[test]
fn all_source_contract_kinds_round_trip_and_resolve_nested_values() {
    let mut contracts = data_contracts();
    contracts.insert(0, c_function_contract());
    contracts.insert(1, scoop_function_contract());
    let mut resolver = resolver();

    for contract in contracts {
        let decoded = decode_canonical::<DecodedSourceNativeExternalContract>(
            &encode(&contract).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut resolver).unwrap(), contract);
    }
}

#[test]
fn source_contract_records_round_trip_and_verify_identity() {
    let function = function_declaration(None);
    let property = property_declaration();
    let records = [
        SourceNativeExternalContractRecord::new(
            SourceNativeExternalContractKey::function(&function).unwrap(),
            c_function_contract(),
        )
        .unwrap(),
        SourceNativeExternalContractRecord::new(
            SourceNativeExternalContractKey::property(&property).unwrap(),
            data_contracts().remove(0),
        )
        .unwrap(),
    ];
    let mut resolver = Resolver { function, property };

    for record in records {
        let decoded = decode_canonical::<DecodedSourceNativeExternalContractRecord>(
            &encode(&record).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut resolver).unwrap(), record);
    }
}

#[test]
fn source_contract_record_rejects_mismatched_owner_binders_and_id() {
    let function = function_declaration(None);
    let property = property_declaration();
    let valid = SourceNativeExternalContractRecord::new(
        SourceNativeExternalContractKey::function(&function).unwrap(),
        c_function_contract(),
    )
    .unwrap();

    let mut mismatched = decode_record(&valid);
    mismatched.contract = decode_contract(&data_contracts().remove(0));
    assert_eq!(
        mismatched.resolve(&mut Resolver {
            function: function.clone(),
            property: property.clone(),
        }),
        Err(SourceNativeExternalResolutionError::Contract(
            SourceNativeContractError::OwnerContractMismatch
        ))
    );

    let binder_contract = SourceNativeExternalContract::Function {
        symbol: source_symbol(),
        library: SourceNativeLibraryBinding::DefaultNativeNamespace,
        abi: SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
            SourceCAbiReturn::Void,
        )),
        calling_convention: SourceCallingConvention::Cdecl,
    };
    let mut binder = decode_record(&valid);
    binder.contract = decode_contract(&binder_contract);
    assert_eq!(
        binder.resolve(&mut Resolver {
            function: function.clone(),
            property: property.clone(),
        }),
        Err(SourceNativeExternalResolutionError::Contract(
            SourceNativeContractError::ContainsBinder
        ))
    );

    let mut bytes = encode(&valid).unwrap();
    assert_eq!(&bytes[..4], &[0xa3, 0x01, 0x58, 0x20]);
    bytes[4] ^= 1;
    let wrong_id = decode_canonical::<DecodedSourceNativeExternalContractRecord>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        wrong_id.resolve(&mut Resolver { function, property }),
        Err(SourceNativeExternalResolutionError::Id(_))
    ));
}

#[test]
fn source_contract_key_rechecks_owner_shape() {
    let function = function_declaration(Some(nominal()));
    let property = property_declaration();
    let owner = SourceNativeExternalOwner::Function(
        PersistentFunctionId::from_source_declaration(&function).unwrap(),
    );
    let decoded = decode_canonical::<DecodedSourceNativeExternalContractKey>(
        &encode(&RawContractKey(owner)).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver { function, property }),
        Err(SourceNativeExternalResolutionError::Contract(
            SourceNativeContractError::ExpectedTopLevelFunction
        ))
    );
}

#[test]
fn source_contract_resolution_rejects_invalid_symbol_and_library() {
    let mut symbol_bytes = encode(&c_function_contract()).unwrap();
    replace_once(&mut symbol_bytes, b"native_entry", b"native\0entry");
    let symbol = decode_canonical::<DecodedSourceNativeExternalContract>(
        &symbol_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        symbol.resolve(&mut resolver()),
        Err(SourceNativeExternalResolutionError::Symbol(
            SourceNativeSymbolError::Nul
        ))
    );

    let contract = SourceNativeExternalContract::ReadOnlyData {
        symbol: source_symbol(),
        library: SourceNativeLibraryBinding::LogicalLibrary(
            CanonicalNativeLibraryName::new("library").unwrap(),
        ),
        storage: nominal(),
    };
    let mut library_bytes = encode(&contract).unwrap();
    replace_once(&mut library_bytes, b"library", b"bad/lib");
    let library = decode_canonical::<DecodedSourceNativeExternalContract>(
        &library_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        library.resolve(&mut resolver()),
        Err(SourceNativeExternalResolutionError::Library(
            CanonicalNativeNameError::ForbiddenCharacter
        ))
    );
}

#[test]
fn source_contract_decoder_rejects_unknown_tags() {
    let mut owner = vec![0xa2, 0x00, 0x03, 0x01, 0x58, 0x20];
    owner.extend([0; 32]);
    assert_unknown::<DecodedSourceNativeExternalOwner>(&owner, 3);
    assert_unknown::<DecodedSourceNativeExternalContract>(b"\xa1\x00\x06", 6);
}

fn decode_record(
    record: &SourceNativeExternalContractRecord,
) -> DecodedSourceNativeExternalContractRecord {
    decode_canonical(&encode(record).unwrap(), DecodeLimits::default()).unwrap()
}

fn decode_contract(contract: &SourceNativeExternalContract) -> DecodedSourceNativeExternalContract {
    decode_canonical(&encode(contract).unwrap(), DecodeLimits::default()).unwrap()
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes, DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag });
}

fn replace_once(bytes: &mut [u8], source: &[u8], replacement: &[u8]) {
    assert_eq!(source.len(), replacement.len());
    let offset = bytes
        .windows(source.len())
        .position(|window| window == source)
        .unwrap();
    bytes[offset..offset + source.len()].copy_from_slice(replacement);
}

fn resolver() -> Resolver {
    Resolver {
        function: function_declaration(None),
        property: property_declaration(),
    }
}

fn c_function_contract() -> SourceNativeExternalContract {
    SourceNativeExternalContract::Function {
        symbol: source_symbol(),
        library: SourceNativeLibraryBinding::DefaultNativeNamespace,
        abi: SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
            vec![nominal()],
            SourceCAbiReturn::Void,
        )),
        calling_convention: SourceCallingConvention::Cdecl,
    }
}

fn scoop_function_contract() -> SourceNativeExternalContract {
    SourceNativeExternalContract::Function {
        symbol: source_symbol(),
        library: SourceNativeLibraryBinding::DefaultNativeNamespace,
        abi: SourceExternFunctionAbi::Scoop {
            signature: SourceScoopAbiFunctionSignature::new(vec![nominal()], nominal()),
            gc_effect: crate::GcEffect::Managed,
        },
        calling_convention: SourceCallingConvention::Cdecl,
    }
}

fn data_contracts() -> Vec<SourceNativeExternalContract> {
    let symbol = source_symbol();
    let library = SourceNativeLibraryBinding::DefaultNativeNamespace;
    let storage = nominal();
    vec![
        SourceNativeExternalContract::ReadOnlyData {
            symbol: symbol.clone(),
            library: library.clone(),
            storage: storage.clone(),
        },
        SourceNativeExternalContract::MutableData {
            symbol: symbol.clone(),
            library: library.clone(),
            storage: storage.clone(),
        },
        SourceNativeExternalContract::ReadOnlyTls {
            symbol: symbol.clone(),
            library: library.clone(),
            storage: storage.clone(),
        },
        SourceNativeExternalContract::MutableTls {
            symbol,
            library,
            storage,
        },
    ]
}

fn source_symbol() -> SourceNativeSymbol {
    SourceNativeSymbol::new("native_entry").unwrap()
}

fn function_declaration(receiver: Option<SignatureTypeKey>) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        source_site(),
        CanonicalIdentifier::new("run").unwrap(),
        0,
        receiver,
        Vec::new(),
    )
}

fn property_declaration() -> SourceDeclarationKey {
    SourceDeclarationKey::property(source_site(), CanonicalIdentifier::new("state").unwrap())
}

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        crate::PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn nominal() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(plain_type())
}

const fn plain_type() -> PersistentTypeId {
    PersistentTypeId([7; 32])
}

const fn generic_type() -> PersistentGenericTypeId {
    PersistentGenericTypeId([8; 32])
}

struct RawContractKey(SourceNativeExternalOwner);

impl scoop_wire::WireEncode for RawContractKey {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.0.encode(encoder)
    }
}
