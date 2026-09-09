use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{
    CanonicalNativeLibraryName, SignatureTypeKeyV1, SourceDeclarationIdentityError,
    SourceDeclarationKeyV1, SourceDeclarationKindV1, SourceNativeSymbolV1,
};
use crate::ids::derive_persistent_id;
use crate::{PersistentFunctionId, PersistentPropertyId, PersistentSourceNativeExternalContractId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceNativeExternalOwnerV1 {
    Function(PersistentFunctionId),
    Property(PersistentPropertyId),
}

impl WireEncodeV1 for SourceNativeExternalOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::Property(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceNativeExternalContractKeyV1 {
    owner: SourceNativeExternalOwnerV1,
}

impl SourceNativeExternalContractKeyV1 {
    pub fn function(key: &SourceDeclarationKeyV1) -> Result<Self, SourceNativeContractError> {
        require_top_level_non_generic(key, SourceDeclarationKindV1::Function)?;
        if key.duplicate_signature().receiver_is_present() {
            return Err(SourceNativeContractError::ExpectedTopLevelFunction);
        }
        let owner = PersistentFunctionId::from_source_declaration(key)
            .map_err(SourceNativeContractError::SourceDeclaration)?;
        Ok(Self {
            owner: SourceNativeExternalOwnerV1::Function(owner),
        })
    }

    pub fn property(key: &SourceDeclarationKeyV1) -> Result<Self, SourceNativeContractError> {
        require_top_level_non_generic(key, SourceDeclarationKindV1::Property)?;
        let owner = PersistentPropertyId::from_source_declaration(key)
            .map_err(SourceNativeContractError::SourceDeclaration)?;
        Ok(Self {
            owner: SourceNativeExternalOwnerV1::Property(owner),
        })
    }

    pub const fn owner(&self) -> SourceNativeExternalOwnerV1 {
        self.owner
    }
}

impl WireEncodeV1 for SourceNativeExternalContractKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.owner.encode(encoder)
    }
}

impl PersistentSourceNativeExternalContractId {
    pub fn from_key(key: &SourceNativeExternalContractKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-source-native-contract-id-v1", key)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceNativeLibraryBindingV1 {
    DefaultNativeNamespace,
    LogicalLibrary(CanonicalNativeLibraryName),
}

impl WireEncodeV1 for SourceNativeLibraryBindingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::DefaultNativeNamespace => encode_empty_sum(encoder, 1),
            Self::LogicalLibrary(name) => encode_value_sum(encoder, 2, name),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceCAbiReturnV1 {
    Void,
    Value(SignatureTypeKeyV1),
}

impl SourceCAbiReturnV1 {
    fn contains_binder(&self) -> bool {
        match self {
            Self::Void => false,
            Self::Value(value) => value.contains_binder(),
        }
    }
}

impl WireEncodeV1 for SourceCAbiReturnV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Void => encode_empty_sum(encoder, 1),
            Self::Value(value) => encode_value_sum(encoder, 2, value),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceCAbiFunctionSignatureV1 {
    parameters: Vec<SignatureTypeKeyV1>,
    result: SourceCAbiReturnV1,
}

impl SourceCAbiFunctionSignatureV1 {
    pub fn new(parameters: Vec<SignatureTypeKeyV1>, result: SourceCAbiReturnV1) -> Self {
        Self { parameters, result }
    }

    pub fn parameters(&self) -> &[SignatureTypeKeyV1] {
        &self.parameters
    }

    pub fn result(&self) -> &SourceCAbiReturnV1 {
        &self.result
    }

    fn contains_binder(&self) -> bool {
        self.parameters
            .iter()
            .any(SignatureTypeKeyV1::contains_binder)
            || self.result.contains_binder()
    }
}

impl WireEncodeV1 for SourceCAbiFunctionSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_signature_types(encoder, &self.parameters)?;
        encoder.field(2)?;
        self.result.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceScoopAbiFunctionSignatureV1 {
    parameters: Vec<SignatureTypeKeyV1>,
    result: SignatureTypeKeyV1,
}

impl SourceScoopAbiFunctionSignatureV1 {
    pub fn new(parameters: Vec<SignatureTypeKeyV1>, result: SignatureTypeKeyV1) -> Self {
        Self { parameters, result }
    }

    pub fn parameters(&self) -> &[SignatureTypeKeyV1] {
        &self.parameters
    }

    pub fn result(&self) -> &SignatureTypeKeyV1 {
        &self.result
    }

    fn contains_binder(&self) -> bool {
        self.parameters
            .iter()
            .any(SignatureTypeKeyV1::contains_binder)
            || self.result.contains_binder()
    }
}

impl WireEncodeV1 for SourceScoopAbiFunctionSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_signature_types(encoder, &self.parameters)?;
        encoder.field(2)?;
        self.result.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GcEffectV1 {
    Managed,
    NoGc,
}

impl WireEncodeV1 for GcEffectV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Managed => 1,
            Self::NoGc => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceCallingConventionV1 {
    Cdecl,
}

impl WireEncodeV1 for SourceCallingConventionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceExternFunctionAbiV1 {
    C(SourceCAbiFunctionSignatureV1),
    Scoop {
        signature: SourceScoopAbiFunctionSignatureV1,
        gc_effect: GcEffectV1,
    },
}

impl SourceExternFunctionAbiV1 {
    fn contains_binder(&self) -> bool {
        match self {
            Self::C(signature) => signature.contains_binder(),
            Self::Scoop { signature, .. } => signature.contains_binder(),
        }
    }
}

impl WireEncodeV1 for SourceExternFunctionAbiV1 {
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
pub enum CallbackModeV1 {
    Reusable,
    OneShot,
}

impl WireEncodeV1 for CallbackModeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Reusable => 1,
            Self::OneShot => 2,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceNativeExternalContractV1 {
    Function {
        symbol: SourceNativeSymbolV1,
        library: SourceNativeLibraryBindingV1,
        abi: SourceExternFunctionAbiV1,
        calling_convention: SourceCallingConventionV1,
    },
    ReadOnlyData {
        symbol: SourceNativeSymbolV1,
        library: SourceNativeLibraryBindingV1,
        storage: SignatureTypeKeyV1,
    },
    MutableData {
        symbol: SourceNativeSymbolV1,
        library: SourceNativeLibraryBindingV1,
        storage: SignatureTypeKeyV1,
    },
    ReadOnlyTls {
        symbol: SourceNativeSymbolV1,
        library: SourceNativeLibraryBindingV1,
        storage: SignatureTypeKeyV1,
    },
    MutableTls {
        symbol: SourceNativeSymbolV1,
        library: SourceNativeLibraryBindingV1,
        storage: SignatureTypeKeyV1,
    },
}

impl SourceNativeExternalContractV1 {
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

impl WireEncodeV1 for SourceNativeExternalContractV1 {
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
pub struct SourceNativeExternalContractRecordV1 {
    id: PersistentSourceNativeExternalContractId,
    key: SourceNativeExternalContractKeyV1,
    contract: SourceNativeExternalContractV1,
}

impl SourceNativeExternalContractRecordV1 {
    pub fn new(
        key: SourceNativeExternalContractKeyV1,
        contract: SourceNativeExternalContractV1,
    ) -> Result<Self, SourceNativeContractError> {
        let owner_is_function = matches!(key.owner, SourceNativeExternalOwnerV1::Function(_));
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

    pub const fn key(&self) -> SourceNativeExternalContractKeyV1 {
        self.key
    }

    pub fn contract(&self) -> &SourceNativeExternalContractV1 {
        &self.contract
    }
}

impl WireEncodeV1 for SourceNativeExternalContractRecordV1 {
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
    key: &SourceDeclarationKeyV1,
    expected: SourceDeclarationKindV1,
) -> Result<(), SourceNativeContractError> {
    if key.declaration_kind() != expected || !key.owners().owners().is_empty() {
        return Err(if expected == SourceDeclarationKindV1::Function {
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
    values: &[SignatureTypeKeyV1],
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
    value: &impl WireEncodeV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncodeV1,
    second: &impl WireEncodeV1,
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
    symbol: &SourceNativeSymbolV1,
    library: &SourceNativeLibraryBindingV1,
    storage: &SignatureTypeKeyV1,
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
        SourceCAbiFunctionSignatureV1, SourceCAbiReturnV1, SourceCallingConventionV1,
        SourceExternFunctionAbiV1, SourceNativeContractError, SourceNativeExternalContractKeyV1,
        SourceNativeExternalContractRecordV1, SourceNativeExternalContractV1,
        SourceNativeLibraryBindingV1,
    };
    use crate::{
        CanonicalIdentifier, ConeIdentity, DeclarationScopeV1, DefinitionOwnerChainV1, PackagePath,
        PersistentSourceNativeExternalContractId, SignatureTypeKeyV1, SourceDeclarationKeyV1,
        SourceDeclarationSiteV1, SourceNativeSymbolV1,
    };

    #[test]
    fn source_contract_key_has_fixed_identity() {
        let declaration = function_declaration(0, None);
        let key = SourceNativeExternalContractKeyV1::function(&declaration).unwrap();
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
        let key = SourceNativeExternalContractKeyV1::function(&declaration).unwrap();
        let contract = SourceNativeExternalContractV1::Function {
            symbol: SourceNativeSymbolV1::new("native_run").unwrap(),
            library: SourceNativeLibraryBindingV1::DefaultNativeNamespace,
            abi: SourceExternFunctionAbiV1::C(SourceCAbiFunctionSignatureV1::new(
                vec![SignatureTypeKeyV1::Binder { depth: 0, index: 0 }],
                SourceCAbiReturnV1::Void,
            )),
            calling_convention: SourceCallingConventionV1::Cdecl,
        };
        assert_eq!(
            SourceNativeExternalContractRecordV1::new(key, contract),
            Err(SourceNativeContractError::ContainsBinder)
        );
    }

    #[test]
    fn callback_source_signature_can_retain_binder() {
        let signature = SourceCAbiFunctionSignatureV1::new(
            vec![SignatureTypeKeyV1::Binder { depth: 0, index: 0 }],
            SourceCAbiReturnV1::Void,
        );
        assert!(encode(&signature).is_ok());
    }

    #[test]
    fn source_contract_owner_rejects_generic_and_receiver() {
        assert_eq!(
            SourceNativeExternalContractKeyV1::function(&function_declaration(1, None)),
            Err(SourceNativeContractError::ExpectedNonGeneric)
        );
        assert_eq!(
            SourceNativeExternalContractKeyV1::function(&function_declaration(
                0,
                Some(SignatureTypeKeyV1::Nominal(crate::PersistentTypeId(
                    ConeIdentity::CORE.0,
                ))),
            )),
            Err(SourceNativeContractError::ExpectedTopLevelFunction)
        );
    }

    fn function_declaration(
        type_parameters: u32,
        receiver: Option<SignatureTypeKeyV1>,
    ) -> SourceDeclarationKeyV1 {
        SourceDeclarationKeyV1::function(
            SourceDeclarationSiteV1::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChainV1::top_level(),
                DeclarationScopeV1::ConeWide,
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
