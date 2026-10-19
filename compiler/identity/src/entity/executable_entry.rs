//! Typed source and generated executable entries.

use crate::{
    CallableBodyKey, CborIdentityRecord, ConeIdentity, DeclarationName, DecodedPersistentId,
    DuplicateSignatureKey, ExactCallableSignature, GeneratedCallableKey, MainCallableBodyId,
    OptionalSignatureType, PersistentCallableBodyId, PersistentFunctionId,
    PersistentGeneratedCallableId, SourceDeclarationKey, SourceDeclarationKind,
    SourceSignatureFingerprint,
};
use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError};
use std::fmt;

mod declaration;
pub use declaration::{DecodedExecutableEntryDeclaration, ExecutableEntryDeclaration};

/// Persistent declaration, signature and body of the selected executable entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableEntryIdentity {
    root_cone: ConeIdentity,
    declaration: ExecutableEntryDeclaration,
    source_signature: ExactCallableSignature,
    source_signature_fingerprint: SourceSignatureFingerprint,
    main: MainCallableBodyId,
}

impl ExecutableEntryIdentity {
    pub fn try_new(
        declaration: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
        source_signature: ExactCallableSignature,
    ) -> Result<Self, ExecutableEntryIdentityError> {
        let key = declaration.key();
        if key.declaration_kind() != SourceDeclarationKind::Function {
            return Err(ExecutableEntryIdentityError::NotFunction);
        }
        if !key.owners().owners().is_empty() {
            return Err(ExecutableEntryIdentityError::NotTopLevel);
        }
        if !matches!(key.name(), DeclarationName::Named(name) if name.as_str() == "main") {
            return Err(ExecutableEntryIdentityError::NotMain);
        }
        if !matches!(
            key.duplicate_signature(),
            DuplicateSignatureKey::Function {
                type_parameter_count: 0,
                receiver: OptionalSignatureType::Absent,
                parameters,
                conditions,
            } if conditions.is_empty() && parameters.len() == source_signature.parameters().len() && parameters.len() <= 1
        ) {
            return Err(ExecutableEntryIdentityError::InvalidDeclarationShape);
        }
        if source_signature.effect() != super::Effect::Ordinary
            || source_signature.receiver().is_present()
        {
            return Err(ExecutableEntryIdentityError::InvalidDeclarationShape);
        }
        let declaration_id = declaration.id();
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
            ExecutableEntryDeclaration::Source(declaration_id).strong_owner(),
        ))
        .map_err(ExecutableEntryIdentityError::Hash)?;
        let source_signature_fingerprint =
            SourceSignatureFingerprint::from_signature(&source_signature)
                .map_err(ExecutableEntryIdentityError::Hash)?;
        Ok(Self {
            root_cone: key.origin(),
            declaration: ExecutableEntryDeclaration::Source(declaration_id),
            source_signature,
            source_signature_fingerprint,
            main: MainCallableBodyId::from_body(body),
        })
    }

    pub fn for_test(
        declaration: &CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>,
        signature: ExactCallableSignature,
    ) -> Result<Self, ExecutableEntryIdentityError> {
        let GeneratedCallableKey::TestEntry { cone } = declaration.key() else {
            return Err(ExecutableEntryIdentityError::NotTestEntry);
        };
        if signature.effect() != super::Effect::Ordinary
            || signature.receiver().is_present()
            || signature.parameters().len() > 1
        {
            return Err(ExecutableEntryIdentityError::InvalidDeclarationShape);
        }
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
            ExecutableEntryDeclaration::Generated(declaration.id()).strong_owner(),
        ))
        .map_err(ExecutableEntryIdentityError::Hash)?;
        let fingerprint = SourceSignatureFingerprint::from_signature(&signature)
            .map_err(ExecutableEntryIdentityError::Hash)?;
        Ok(Self {
            root_cone: *cone,
            declaration: ExecutableEntryDeclaration::Generated(declaration.id()),
            source_signature: signature,
            source_signature_fingerprint: fingerprint,
            main: MainCallableBodyId::from_body(body),
        })
    }

    pub const fn root_cone(&self) -> ConeIdentity {
        self.root_cone
    }

    pub const fn declaration(&self) -> ExecutableEntryDeclaration {
        self.declaration
    }

    pub const fn source_signature(&self) -> &ExactCallableSignature {
        &self.source_signature
    }

    pub const fn source_signature_fingerprint(&self) -> SourceSignatureFingerprint {
        self.source_signature_fingerprint
    }

    pub const fn main(&self) -> MainCallableBodyId {
        self.main
    }
}

impl WireEncode for ExecutableEntryIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.root_cone.encode(encoder)?;
        encoder.field(2)?;
        self.declaration.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.main.encode(encoder)
    }
}

/// Untrusted wire form of [`ExecutableEntryIdentity`].
///
/// Readers resolve the source or generated declaration and exact types, then check the
/// stored identities and fingerprint against the resolved signature.
#[derive(Debug)]
pub struct DecodedExecutableEntryIdentity {
    root_cone: DecodedPersistentId<ConeIdentity>,
    declaration: DecodedExecutableEntryDeclaration,
    source_signature: crate::DecodedExactCallableSignature,
    source_signature_fingerprint: DecodedPersistentId<SourceSignatureFingerprint>,
    main: DecodedPersistentId<PersistentCallableBodyId>,
}

impl DecodedExecutableEntryIdentity {
    pub const fn source_signature(&self) -> &crate::DecodedExactCallableSignature {
        &self.source_signature
    }

    pub const fn declaration(&self) -> DecodedExecutableEntryDeclaration {
        self.declaration
    }
}

impl WireEncode for DecodedExecutableEntryIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.root_cone.encode(encoder)?;
        encoder.field(2)?;
        self.declaration.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.main.encode(encoder)
    }
}

impl WireDecode for DecodedExecutableEntryIdentity {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            root_cone: decoder.field(1, DecodedPersistentId::decode)?,
            declaration: decoder.field(2, DecodedExecutableEntryDeclaration::decode)?,
            source_signature: decoder.field(3, crate::DecodedExactCallableSignature::decode)?,
            source_signature_fingerprint: decoder.field(4, DecodedPersistentId::decode)?,
            main: decoder.field(5, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableEntryIdentityError {
    NotFunction,
    NotTestEntry,
    NotTopLevel,
    NotMain,
    InvalidDeclarationShape,
    Hash(HashError),
}

impl fmt::Display for ExecutableEntryIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid executable entry identity: {self:?}")
    }
}

impl std::error::Error for ExecutableEntryIdentityError {}

#[cfg(test)]
mod tests;
