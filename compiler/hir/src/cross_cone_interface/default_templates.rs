use scoop_identity::{CallableTemplateOrigin, DecodedCallableTemplateOrigin};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::CallableDeclarationIdResolver;

mod binder_uses;
mod locals;
mod receiver;
mod roots;
mod value_parameters;

pub use binder_uses::{
    BinderUseListBuildError, BinderUseListValidationError, CanonicalBinderUseListV1,
    DecodedCanonicalBinderUseListV1,
};
pub use locals::{
    CanonicalTemplateLocalTableV1, DecodedCanonicalTemplateLocalTableV1,
    DecodedTemplateLocalDefinitionV1, DecodedTemplateLocalRecordV1, TemplateLocalDefinitionV1,
    TemplateLocalIndexResolver, TemplateLocalLookupError, TemplateLocalRecordBuildError,
    TemplateLocalRecordResolutionError, TemplateLocalRecordV1, TemplateLocalReferenceResolver,
    TemplateLocalSelectorResolver, TemplateLocalTableBuildError, TemplateLocalTableValidationError,
};
pub use receiver::{
    DecodedOptionalTemplateReceiverV1, DecodedTemplateReceiverV1,
    IndexedOptionalTemplateReceiverV1, IndexedTemplateReceiverV1, OptionalTemplateReceiverV1,
    TemplateReceiverBuildError, TemplateReceiverIndexError, TemplateReceiverResolutionError,
    TemplateReceiverV1,
};
pub use roots::{
    DecodedPersistentLexicalRootV1, PersistentLexicalRootBuildError, PersistentLexicalRootResolver,
    PersistentLexicalRootV1,
};
pub use value_parameters::{
    CanonicalTemplateValueParametersV1, DecodedCanonicalTemplateValueParametersV1,
    DecodedTemplateValueParameterV1, IndexedCanonicalTemplateValueParametersV1,
    IndexedTemplateValueParameterV1, TemplateValueParameterBuildError,
    TemplateValueParameterIndexError, TemplateValueParameterListBuildError,
    TemplateValueParameterListIndexError, TemplateValueParameterListValidationError,
    TemplateValueParameterResolutionError, TemplateValueParameterV1,
};

/// Section-local identity of one exported default template.
///
/// This key is stable because its owner is persistent and its parameter
/// position is declaration-relative. It is deliberately not a new persistent
/// entity id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDefaultTemplateKeyV1 {
    owner: CallableTemplateOrigin,
    parameter_position: u32,
}

impl ExportDefaultTemplateKeyV1 {
    pub const fn new(owner: CallableTemplateOrigin, parameter_position: u32) -> Self {
        Self {
            owner,
            parameter_position,
        }
    }

    pub const fn owner(self) -> CallableTemplateOrigin {
        self.owner
    }

    pub const fn parameter_position(self) -> u32 {
        self.parameter_position
    }
}

impl WireEncode for ExportDefaultTemplateKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.parameter_position))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedExportDefaultTemplateKeyV1 {
    owner: DecodedCallableTemplateOrigin,
    parameter_position: u32,
}

impl DecodedExportDefaultTemplateKeyV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<ExportDefaultTemplateKeyV1, E>
    where
        R: CallableDeclarationIdResolver<E>,
    {
        self.owner
            .resolve(resolver)
            .map(|owner| ExportDefaultTemplateKeyV1::new(owner, self.parameter_position))
    }
}

impl WireEncode for DecodedExportDefaultTemplateKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.parameter_position))
    }
}

impl WireDecode for DecodedExportDefaultTemplateKeyV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            parameter_position: decoder.field(2, Decoder::u32)?,
        })
    }
}

#[cfg(test)]
mod tests;
