use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    CanonicalNativeLibraryName, SignatureTypeKey, SourceDeclarationIdentityError,
    SourceDeclarationKey, SourceDeclarationKind, SourceNativeSymbol,
};
use crate::ids::derive_persistent_id;
use crate::{PersistentFunctionId, PersistentPropertyId, PersistentSourceNativeExternalContractId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceNativeExternalOwner {
    Function(PersistentFunctionId),
    Property(PersistentPropertyId),
}

impl WireEncode for SourceNativeExternalOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::Property(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceNativeExternalContractKey {
    owner: SourceNativeExternalOwner,
}

impl SourceNativeExternalContractKey {
    pub fn function(key: &SourceDeclarationKey) -> Result<Self, SourceNativeContractError> {
        require_top_level_non_generic(key, SourceDeclarationKind::Function)?;
        if key.duplicate_signature().receiver_is_present() {
            return Err(SourceNativeContractError::ExpectedTopLevelFunction);
        }
        let owner = PersistentFunctionId::from_source_declaration(key)
            .map_err(SourceNativeContractError::SourceDeclaration)?;
        Ok(Self {
            owner: SourceNativeExternalOwner::Function(owner),
        })
    }

    pub fn property(key: &SourceDeclarationKey) -> Result<Self, SourceNativeContractError> {
        require_top_level_non_generic(key, SourceDeclarationKind::Property)?;
        let owner = PersistentPropertyId::from_source_declaration(key)
            .map_err(SourceNativeContractError::SourceDeclaration)?;
        Ok(Self {
            owner: SourceNativeExternalOwner::Property(owner),
        })
    }

    pub const fn owner(&self) -> SourceNativeExternalOwner {
        self.owner
    }
}

impl WireEncode for SourceNativeExternalContractKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.owner.encode(encoder)
    }
}

impl PersistentSourceNativeExternalContractId {
    pub fn from_key(key: &SourceNativeExternalContractKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-source-native-contract-id-v1", key)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceNativeLibraryBinding {
    DefaultNativeNamespace,
    LogicalLibrary(CanonicalNativeLibraryName),
}

impl WireEncode for SourceNativeLibraryBinding {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::DefaultNativeNamespace => encode_empty_sum(encoder, 1),
            Self::LogicalLibrary(name) => encode_value_sum(encoder, 2, name),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceCAbiReturn {
    Void,
    Value(SignatureTypeKey),
}

impl SourceCAbiReturn {
    fn contains_binder(&self) -> bool {
        match self {
            Self::Void => false,
            Self::Value(value) => value.contains_binder(),
        }
    }
}

impl WireEncode for SourceCAbiReturn {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Void => encode_empty_sum(encoder, 1),
            Self::Value(value) => encode_value_sum(encoder, 2, value),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceCAbiFunctionSignature {
    parameters: Vec<SignatureTypeKey>,
    result: SourceCAbiReturn,
}

impl SourceCAbiFunctionSignature {
    pub fn new(parameters: Vec<SignatureTypeKey>, result: SourceCAbiReturn) -> Self {
        Self { parameters, result }
    }

    pub fn parameters(&self) -> &[SignatureTypeKey] {
        &self.parameters
    }

    pub fn result(&self) -> &SourceCAbiReturn {
        &self.result
    }

    fn contains_binder(&self) -> bool {
        self.parameters
            .iter()
            .any(SignatureTypeKey::contains_binder)
            || self.result.contains_binder()
    }
}

impl WireEncode for SourceCAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_signature_types(encoder, &self.parameters)?;
        encoder.field(2)?;
        self.result.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceScoopAbiFunctionSignature {
    parameters: Vec<SignatureTypeKey>,
    result: SignatureTypeKey,
}

impl SourceScoopAbiFunctionSignature {
    pub fn new(parameters: Vec<SignatureTypeKey>, result: SignatureTypeKey) -> Self {
        Self { parameters, result }
    }

    pub fn parameters(&self) -> &[SignatureTypeKey] {
        &self.parameters
    }

    pub fn result(&self) -> &SignatureTypeKey {
        &self.result
    }

    fn contains_binder(&self) -> bool {
        self.parameters
            .iter()
            .any(SignatureTypeKey::contains_binder)
            || self.result.contains_binder()
    }
}

impl WireEncode for SourceScoopAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_signature_types(encoder, &self.parameters)?;
        encoder.field(2)?;
        self.result.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GcEffect {
    Managed,
    NoGc,
}

impl WireEncode for GcEffect {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Managed => 1,
            Self::NoGc => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceCallingConvention {
    Cdecl,
}

impl WireEncode for SourceCallingConvention {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceExternFunctionAbi {
    C(SourceCAbiFunctionSignature),
    Scoop {
        signature: SourceScoopAbiFunctionSignature,
        gc_effect: GcEffect,
    },
}

impl SourceExternFunctionAbi {
    fn contains_binder(&self) -> bool {
        match self {
            Self::C(signature) => signature.contains_binder(),
            Self::Scoop { signature, .. } => signature.contains_binder(),
        }
    }
}

impl WireEncode for SourceExternFunctionAbi {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::C(signature) => encode_value_sum(encoder, 1, signature),
            Self::Scoop {
                signature,
                gc_effect,
            } => encode_two_value_sum(encoder, 2, signature, gc_effect),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallbackMode {
    Reusable,
    OneShot,
}

impl WireEncode for CallbackMode {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Reusable => 1,
            Self::OneShot => 2,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceNativeExternalContract {
    Function {
        symbol: SourceNativeSymbol,
        library: SourceNativeLibraryBinding,
        abi: SourceExternFunctionAbi,
        calling_convention: SourceCallingConvention,
    },
    ReadOnlyData {
        symbol: SourceNativeSymbol,
        library: SourceNativeLibraryBinding,
        storage: SignatureTypeKey,
    },
    MutableData {
        symbol: SourceNativeSymbol,
        library: SourceNativeLibraryBinding,
        storage: SignatureTypeKey,
    },
    ReadOnlyTls {
        symbol: SourceNativeSymbol,
        library: SourceNativeLibraryBinding,
        storage: SignatureTypeKey,
    },
    MutableTls {
        symbol: SourceNativeSymbol,
        library: SourceNativeLibraryBinding,
        storage: SignatureTypeKey,
    },
}

impl SourceNativeExternalContract {
    fn contains_binder(&self) -> bool {
        match self {
            Self::Function { abi, .. } => abi.contains_binder(),
            Self::ReadOnlyData { storage, .. }
            | Self::MutableData { storage, .. }
            | Self::ReadOnlyTls { storage, .. }
            | Self::MutableTls { storage, .. } => storage.contains_binder(),
        }
    }

    fn is_function(&self) -> bool {
        matches!(self, Self::Function { .. })
    }
}

impl WireEncode for SourceNativeExternalContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function {
                symbol,
                library,
                abi,
                calling_convention,
            } => {
                encoder.map(5)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                symbol.encode(encoder)?;
                encoder.field(2)?;
                library.encode(encoder)?;
                encoder.field(3)?;
                abi.encode(encoder)?;
                encoder.field(4)?;
                calling_convention.encode(encoder)
            }
            Self::ReadOnlyData {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 2, symbol, library, storage),
            Self::MutableData {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 3, symbol, library, storage),
            Self::ReadOnlyTls {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 4, symbol, library, storage),
            Self::MutableTls {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 5, symbol, library, storage),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceNativeExternalContractRecord {
    id: PersistentSourceNativeExternalContractId,
    key: SourceNativeExternalContractKey,
    contract: SourceNativeExternalContract,
}

impl SourceNativeExternalContractRecord {
    pub fn new(
        key: SourceNativeExternalContractKey,
        contract: SourceNativeExternalContract,
    ) -> Result<Self, SourceNativeContractError> {
        let owner_is_function = matches!(key.owner, SourceNativeExternalOwner::Function(_));
        if owner_is_function != contract.is_function() {
            return Err(SourceNativeContractError::OwnerContractMismatch);
        }
        if contract.contains_binder() {
            return Err(SourceNativeContractError::ContainsBinder);
        }
        let id = PersistentSourceNativeExternalContractId::from_key(&key)
            .map_err(SourceNativeContractError::Hash)?;
        Ok(Self { id, key, contract })
    }

    pub const fn id(&self) -> PersistentSourceNativeExternalContractId {
        self.id
    }

    pub const fn key(&self) -> SourceNativeExternalContractKey {
        self.key
    }

    pub fn contract(&self) -> &SourceNativeExternalContract {
        &self.contract
    }
}

impl WireEncode for SourceNativeExternalContractRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.key.encode(encoder)?;
        encoder.field(3)?;
        self.contract.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceNativeContractError {
    ExpectedTopLevelFunction,
    ExpectedTopLevelProperty,
    ExpectedNonGeneric,
    OwnerContractMismatch,
    ContainsBinder,
    SourceDeclaration(SourceDeclarationIdentityError),
    Hash(HashError),
}

impl fmt::Display for SourceNativeContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedTopLevelFunction => {
                formatter.write_str("source native function owner must be a top-level function")
            }
            Self::ExpectedTopLevelProperty => {
                formatter.write_str("source native data owner must be a top-level property")
            }
            Self::ExpectedNonGeneric => {
                formatter.write_str("source native contract owner must be non-generic")
            }
            Self::OwnerContractMismatch => {
                formatter.write_str("source native contract kind does not match its owner")
            }
            Self::ContainsBinder => {
                formatter.write_str("source native external contract must be binder-free")
            }
            Self::SourceDeclaration(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SourceNativeContractError {}

fn require_top_level_non_generic(
    key: &SourceDeclarationKey,
    expected: SourceDeclarationKind,
) -> Result<(), SourceNativeContractError> {
    if key.declaration_kind() != expected || !key.owners().owners().is_empty() {
        return Err(if expected == SourceDeclarationKind::Function {
            SourceNativeContractError::ExpectedTopLevelFunction
        } else {
            SourceNativeContractError::ExpectedTopLevelProperty
        });
    }
    if key.duplicate_signature().type_parameter_count() != 0 {
        return Err(SourceNativeContractError::ExpectedNonGeneric);
    }
    Ok(())
}

fn encode_signature_types(
    encoder: &mut Encoder,
    values: &[SignatureTypeKey],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
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

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

fn encode_data_contract(
    encoder: &mut Encoder,
    tag: u64,
    symbol: &SourceNativeSymbol,
    library: &SourceNativeLibraryBinding,
    storage: &SignatureTypeKey,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    symbol.encode(encoder)?;
    encoder.field(2)?;
    library.encode(encoder)?;
    encoder.field(3)?;
    storage.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        SourceCAbiFunctionSignature, SourceCAbiReturn, SourceCallingConvention,
        SourceExternFunctionAbi, SourceNativeContractError, SourceNativeExternalContract,
        SourceNativeExternalContractKey, SourceNativeExternalContractRecord,
        SourceNativeLibraryBinding,
    };
    use crate::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
        PersistentSourceNativeExternalContractId, SignatureTypeKey, SourceDeclarationKey,
        SourceDeclarationSite, SourceNativeSymbol,
    };

    #[test]
    fn source_contract_key_has_fixed_identity() {
        let declaration = function_declaration(0, None);
        let key = SourceNativeExternalContractKey::function(&declaration).unwrap();
        assert_eq!(
            hex(&encode(&key).unwrap()),
            "a101a20001015820ae31f339e136552be928619d00e3aa3b00056c8c4cbec480b343ad1350517e89"
        );
        assert_eq!(
            PersistentSourceNativeExternalContractId::from_key(&key)
                .unwrap()
                .to_string(),
            "9704aac7dd16c874573760ac30af0fe6d4872f0b4a965dc264bb43c28f152811"
        );
    }

    #[test]
    fn source_external_record_rejects_binder() {
        let declaration = function_declaration(0, None);
        let key = SourceNativeExternalContractKey::function(&declaration).unwrap();
        let contract = SourceNativeExternalContract::Function {
            symbol: SourceNativeSymbol::new("native_run").unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi: SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
                vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
                SourceCAbiReturn::Void,
            )),
            calling_convention: SourceCallingConvention::Cdecl,
        };
        assert_eq!(
            SourceNativeExternalContractRecord::new(key, contract),
            Err(SourceNativeContractError::ContainsBinder)
        );
    }

    #[test]
    fn callback_source_signature_can_retain_binder() {
        let signature = SourceCAbiFunctionSignature::new(
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
            SourceCAbiReturn::Void,
        );
        assert!(encode(&signature).is_ok());
    }

    #[test]
    fn source_contract_owner_rejects_generic_and_receiver() {
        assert_eq!(
            SourceNativeExternalContractKey::function(&function_declaration(1, None)),
            Err(SourceNativeContractError::ExpectedNonGeneric)
        );
        assert_eq!(
            SourceNativeExternalContractKey::function(&function_declaration(
                0,
                Some(SignatureTypeKey::Nominal(crate::PersistentTypeId(
                    ConeIdentity::CORE.0,
                ))),
            )),
            Err(SourceNativeContractError::ExpectedTopLevelFunction)
        );
    }

    fn function_declaration(
        type_parameters: u32,
        receiver: Option<SignatureTypeKey>,
    ) -> SourceDeclarationKey {
        SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("run").unwrap(),
            type_parameters,
            receiver,
            Vec::new(),
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
