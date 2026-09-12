use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    CanonicalCAbiFunctionSignature, CanonicalCStorageType, CanonicalScoopAbiFunctionSignature,
    NativeExternalSymbolKey, NativeLibraryBinding, TargetCallingConvention,
};
use crate::ids::derive_persistent_id;
use crate::{
    NativeExternalContractFingerprint, PersistentNativeExternalSymbolId,
    PersistentSourceNativeExternalContractId,
};

mod decode;

pub use decode::{
    DecodedNativeExternAbi, DecodedNativeExternalContract, DecodedNativeExternalContractRecord,
    NativeExternalContractFingerprintError, NativeExternalContractResolutionError,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeExternAbi {
    C(CanonicalCAbiFunctionSignature),
    Scoop(CanonicalScoopAbiFunctionSignature),
}

impl WireEncode for NativeExternAbi {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::C(signature) => encode_value_sum(encoder, 1, signature),
            Self::Scoop(signature) => encode_value_sum(encoder, 2, signature),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeExternalContract {
    Function {
        library: NativeLibraryBinding,
        abi: NativeExternAbi,
        calling_convention: TargetCallingConvention,
    },
    ReadOnlyData {
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    },
    MutableData {
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    },
    ReadOnlyTls {
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    },
    MutableTls {
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    },
}

impl NativeExternalContract {
    pub fn c_function(
        library: NativeLibraryBinding,
        signature: CanonicalCAbiFunctionSignature,
    ) -> Self {
        let calling_convention = signature.calling_convention();
        Self::Function {
            library,
            abi: NativeExternAbi::C(signature),
            calling_convention,
        }
    }

    pub fn scoop_function(
        library: NativeLibraryBinding,
        signature: CanonicalScoopAbiFunctionSignature,
        calling_convention: TargetCallingConvention,
    ) -> Self {
        Self::Function {
            library,
            abi: NativeExternAbi::Scoop(signature),
            calling_convention,
        }
    }

    pub const fn read_only_data(
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    ) -> Self {
        Self::ReadOnlyData { library, storage }
    }

    pub const fn mutable_data(
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    ) -> Self {
        Self::MutableData { library, storage }
    }

    pub const fn read_only_tls(
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    ) -> Self {
        Self::ReadOnlyTls { library, storage }
    }

    pub const fn mutable_tls(
        library: NativeLibraryBinding,
        storage: CanonicalCStorageType,
    ) -> Self {
        Self::MutableTls { library, storage }
    }
}

impl WireEncode for NativeExternalContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function {
                library,
                abi,
                calling_convention,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                library.encode(encoder)?;
                encoder.field(2)?;
                abi.encode(encoder)?;
                encoder.field(3)?;
                calling_convention.encode(encoder)
            }
            Self::ReadOnlyData { library, storage } => {
                encode_data_contract(encoder, 2, library, storage)
            }
            Self::MutableData { library, storage } => {
                encode_data_contract(encoder, 3, library, storage)
            }
            Self::ReadOnlyTls { library, storage } => {
                encode_data_contract(encoder, 4, library, storage)
            }
            Self::MutableTls { library, storage } => {
                encode_data_contract(encoder, 5, library, storage)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeExternalContractFingerprintInput {
    symbol_id: PersistentNativeExternalSymbolId,
    contract: NativeExternalContract,
}

impl NativeExternalContractFingerprintInput {
    pub const fn new(
        symbol_id: PersistentNativeExternalSymbolId,
        contract: NativeExternalContract,
    ) -> Self {
        Self {
            symbol_id,
            contract,
        }
    }

    pub const fn symbol_id(&self) -> PersistentNativeExternalSymbolId {
        self.symbol_id
    }

    pub const fn contract(&self) -> &NativeExternalContract {
        &self.contract
    }
}

impl WireEncode for NativeExternalContractFingerprintInput {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.symbol_id.encode(encoder)?;
        encoder.field(2)?;
        self.contract.encode(encoder)
    }
}

impl NativeExternalContractFingerprint {
    pub fn from_input(input: &NativeExternalContractFingerprintInput) -> Result<Self, HashError> {
        derive_persistent_id("scoop-native-external-contract-v1", input)
    }

    pub fn from_symbol_and_contract(
        symbol_id: PersistentNativeExternalSymbolId,
        contract: NativeExternalContract,
    ) -> Result<Self, HashError> {
        Self::from_input(&NativeExternalContractFingerprintInput::new(
            symbol_id, contract,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExternalContractRecord {
    source: PersistentSourceNativeExternalContractId,
    symbol_id: PersistentNativeExternalSymbolId,
    symbol_key: NativeExternalSymbolKey,
    fingerprint: NativeExternalContractFingerprint,
    contract: NativeExternalContract,
}

impl NativeExternalContractRecord {
    pub fn new(
        source: PersistentSourceNativeExternalContractId,
        symbol_key: NativeExternalSymbolKey,
        contract: NativeExternalContract,
    ) -> Result<Self, HashError> {
        let symbol_id = PersistentNativeExternalSymbolId::from_key(&symbol_key)?;
        let fingerprint = NativeExternalContractFingerprint::from_symbol_and_contract(
            symbol_id,
            contract.clone(),
        )?;
        Ok(Self {
            source,
            symbol_id,
            symbol_key,
            fingerprint,
            contract,
        })
    }

    pub const fn source(&self) -> PersistentSourceNativeExternalContractId {
        self.source
    }

    pub const fn symbol_id(&self) -> PersistentNativeExternalSymbolId {
        self.symbol_id
    }

    pub const fn fingerprint(&self) -> NativeExternalContractFingerprint {
        self.fingerprint
    }

    pub fn symbol_key(&self) -> &NativeExternalSymbolKey {
        &self.symbol_key
    }

    pub fn contract(&self) -> &NativeExternalContract {
        &self.contract
    }

    pub(crate) const fn from_verified(
        source: PersistentSourceNativeExternalContractId,
        symbol_id: PersistentNativeExternalSymbolId,
        symbol_key: NativeExternalSymbolKey,
        fingerprint: NativeExternalContractFingerprint,
        contract: NativeExternalContract,
    ) -> Self {
        Self {
            source,
            symbol_id,
            symbol_key,
            fingerprint,
            contract,
        }
    }
}

impl WireEncode for NativeExternalContractRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.symbol_id.encode(encoder)?;
        encoder.field(3)?;
        self.symbol_key.encode(encoder)?;
        encoder.field(4)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.contract.encode(encoder)
    }
}

fn encode_data_contract(
    encoder: &mut Encoder,
    tag: u64,
    library: &NativeLibraryBinding,
    storage: &CanonicalCStorageType,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    library.encode(encoder)?;
    encoder.field(2)?;
    storage.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{NativeExternalContract, NativeExternalContractRecord};
    use crate::{
        CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, CanonicalCStorageType, ConeIdentity,
        NativeExternalContractFingerprint, NativeExternalSymbolKey, NativeLibraryBinding,
        PersistentExactTypeId, PersistentNativeExternalSymbolId,
        PersistentSourceNativeExternalContractId, SourceNativeSymbol,
    };

    #[test]
    fn target_native_data_contract_has_fixed_fingerprint() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let symbol_key = NativeExternalSymbolKey::darwin_macho_external(
            &SourceNativeSymbol::new("foo").unwrap(),
        )
        .unwrap();
        let symbol_id = PersistentNativeExternalSymbolId::from_key(&symbol_key).unwrap();
        let contract = NativeExternalContract::read_only_data(
            NativeLibraryBinding::DefaultNativeNamespace,
            CanonicalCStorageType::Boolean { exact_type: exact },
        );

        assert_eq!(
            hex(&encode(&contract).unwrap()),
            format!("a3000201a1000102a20002015820{exact}")
        );
        assert_eq!(
            NativeExternalContractFingerprint::from_symbol_and_contract(
                symbol_id,
                contract.clone(),
            )
            .unwrap()
            .to_string(),
            "fa2e22f22dc94d1935eb067ca90f6b92c493efb81544d7e376d20f9b51efbf01"
        );

        let record = NativeExternalContractRecord::new(
            PersistentSourceNativeExternalContractId(ConeIdentity::SINGLE_FILE.0),
            symbol_key,
            contract,
        )
        .unwrap();
        assert_eq!(record.symbol_id(), symbol_id);
        assert_eq!(
            record.fingerprint().to_string(),
            "fa2e22f22dc94d1935eb067ca90f6b92c493efb81544d7e376d20f9b51efbf01"
        );
    }

    #[test]
    fn target_native_function_contract_derives_the_outer_convention() {
        let contract = NativeExternalContract::c_function(
            NativeLibraryBinding::DefaultNativeNamespace,
            CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
        );

        assert_eq!(
            hex(&encode(&contract).unwrap()),
            "a4000101a1000102a2000101a30101028003a100010301"
        );
    }

    #[test]
    fn target_native_data_and_tls_kinds_have_distinct_tags() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let storage = CanonicalCStorageType::Boolean { exact_type: exact };
        let library = NativeLibraryBinding::DefaultNativeNamespace;
        let contracts = [
            NativeExternalContract::read_only_data(library, storage),
            NativeExternalContract::mutable_data(library, storage),
            NativeExternalContract::read_only_tls(library, storage),
            NativeExternalContract::mutable_tls(library, storage),
        ];

        for (contract, expected_tag) in contracts.iter().zip(2_u8..=5) {
            let encoded = encode(contract).unwrap();
            assert_eq!(encoded[0], 0xa3);
            assert_eq!(encoded[1], 0x00);
            assert_eq!(encoded[2], expected_tag);
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
