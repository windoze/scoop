use scoop_identity::{CallableTemplateOrigin, DecodedCallableTemplateOrigin};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::CallableDeclarationIdResolver;

mod binder_uses;
mod body;
mod locals;
mod receiver;
mod roots;
mod value_parameters;

pub use binder_uses::{
    BinderUseListBuildError, BinderUseListSemanticValidationError, BinderUseListValidationError,
    CanonicalBinderUseListV1, DecodedCanonicalBinderUseListV1,
};
pub use body::{
    DecodedDefaultAnonymousFunctionV1, DecodedDefaultArrayAssemblyPartV1,
    DecodedDefaultArrayAssemblyV1, DecodedDefaultAssignTargetV1,
    DecodedDefaultBindingClassComponentV1, DecodedDefaultBindingLeafV1,
    DecodedDefaultBindingProjectionV1, DecodedDefaultBindingShapeV1,
    DecodedDefaultBindingStructFieldV1, DecodedDefaultBindingTemporaryV1,
    DecodedDefaultBoundCallableRefV1, DecodedDefaultBoundCallableSourceV1,
    DecodedDefaultCallableBodyTypeArgumentsV1, DecodedDefaultCallableDeclarationV1,
    DecodedDefaultCallableRefV1, DecodedDefaultCallableReferenceTargetV1,
    DecodedDefaultCallableReferenceV1, DecodedDefaultCaptureV1, DecodedDefaultClassConstructorIdV1,
    DecodedDefaultConstructorRefV1, DecodedDefaultEnumVariantFieldRefV1,
    DecodedDefaultEnumVariantRefV1, DecodedDefaultExpressionV1, DecodedDefaultFieldRefV1,
    DecodedDefaultIntegerArgumentsV1, DecodedDefaultIntegerOperationV1, DecodedDefaultLambdaV1,
    DecodedDefaultLiteralEqualityV1, DecodedDefaultLocalFunctionV1, DecodedDefaultMethodCalleeV1,
    DecodedDefaultPatternFieldV1, DecodedDefaultPatternV1, DecodedDefaultPlaceV1,
    DecodedDefaultStringOwnerV1, DecodedOptionalDefaultExpressionV1, DefaultAnonymousFunctionV1,
    DefaultArrayAccessKindV1, DefaultArrayAssemblyBuildError, DefaultArrayAssemblyPartV1,
    DefaultArrayAssemblyV1, DefaultAssignTargetIndexError, DefaultAssignTargetResolutionError,
    DefaultAssignTargetV1, DefaultBinaryOperatorV1, DefaultBinderRefV1,
    DefaultBindingClassComponentV1, DefaultBindingLeafIndexError,
    DefaultBindingLeafResolutionError, DefaultBindingLeafV1, DefaultBindingProjectionBuildError,
    DefaultBindingProjectionResolutionError, DefaultBindingProjectionV1,
    DefaultBindingProjectionViewV1, DefaultBindingShapeBuildError, DefaultBindingShapeIndexError,
    DefaultBindingShapeResolutionError, DefaultBindingShapeV1, DefaultBindingShapeViewV1,
    DefaultBindingStructFieldV1, DefaultBindingTemporaryIndexError,
    DefaultBindingTemporaryResolutionError, DefaultBindingTemporaryV1,
    DefaultBoundCallableRefResolutionError, DefaultBoundCallableRefV1,
    DefaultBoundCallableSourceResolutionError, DefaultBoundCallableSourceV1,
    DefaultCallableBodyTypeArgumentsBuildError, DefaultCallableBodyTypeArgumentsResolutionError,
    DefaultCallableBodyTypeArgumentsV1, DefaultCallableDeclarationV1, DefaultCallableRefBuildError,
    DefaultCallableRefResolutionError, DefaultCallableRefV1, DefaultCallableReferenceBuildError,
    DefaultCallableReferenceIndexError, DefaultCallableReferenceResolutionError,
    DefaultCallableReferenceResolver, DefaultCallableReferenceTargetV1, DefaultCallableReferenceV1,
    DefaultCaptureIndexError, DefaultCaptureResolutionError, DefaultCaptureV1,
    DefaultClassConstructorIdResolver, DefaultClassConstructorIdV1,
    DefaultConstructorRefResolutionError, DefaultConstructorRefV1,
    DefaultConstructorReferenceResolver, DefaultEnumVariantFieldRefResolutionError,
    DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefResolutionError, DefaultEnumVariantRefV1,
    DefaultExpressionBuildError, DefaultExpressionIndexError, DefaultExpressionKindV1,
    DefaultExpressionReferenceResolver, DefaultExpressionResolutionError, DefaultExpressionV1,
    DefaultFieldRefResolutionError, DefaultFieldRefV1, DefaultFieldReferenceResolver,
    DefaultForeignCallbackOperationV1, DefaultIntegerArgumentsV1, DefaultIntegerDivRemV1,
    DefaultIntegerKindV1, DefaultIntegerOperationResolutionError, DefaultIntegerOperationV1,
    DefaultLambdaV1, DefaultLexicalCallableBuildError, DefaultLexicalCallableIndexError,
    DefaultLexicalCallableResolutionError, DefaultLiteralEqualityResolutionError,
    DefaultLiteralEqualityV1, DefaultLocalFunctionBuildError, DefaultLocalFunctionIndexError,
    DefaultLocalFunctionResolutionError, DefaultLocalFunctionV1,
    DefaultMethodCalleeResolutionError, DefaultMethodCalleeV1,
    DefaultNestedCallableReferenceResolver, DefaultNoGcIntegerOperationV1,
    DefaultPatternBuildError, DefaultPatternFieldV1, DefaultPatternIndexError,
    DefaultPatternReferenceResolver, DefaultPatternResolutionError, DefaultPatternV1,
    DefaultPatternViewV1, DefaultPlaceIndexError, DefaultPlaceResolutionError, DefaultPlaceV1,
    DefaultPrimitiveBinaryKindV1, DefaultPrimitiveUnaryKindV1, DefaultStringOwnerResolutionError,
    DefaultStringOwnerV1, DefaultUnaryOperatorV1, IndexedDefaultAnonymousFunctionV1,
    IndexedDefaultAssignTargetV1, IndexedDefaultBindingLeafV1, IndexedDefaultBindingShapeV1,
    IndexedDefaultBindingTemporaryV1, IndexedDefaultCallableReferenceV1, IndexedDefaultCaptureV1,
    IndexedDefaultExpressionV1, IndexedDefaultLambdaV1, IndexedDefaultLocalFunctionV1,
    IndexedDefaultPatternV1, IndexedDefaultPlaceV1, OptionalDefaultExpressionV1,
};
pub use locals::{
    CanonicalTemplateLocalTableV1, DecodedCanonicalTemplateLocalTableV1,
    DecodedTemplateLocalDefinitionV1, DecodedTemplateLocalRecordV1, TemplateLocalDefinitionV1,
    TemplateLocalIndexResolver, TemplateLocalLookupError, TemplateLocalRecordBuildError,
    TemplateLocalRecordResolutionError, TemplateLocalRecordV1, TemplateLocalReferenceResolver,
    TemplateLocalScopeValidationError, TemplateLocalSelectorResolver, TemplateLocalTableBuildError,
    TemplateLocalTableValidationError,
};
pub use receiver::{
    DecodedOptionalTemplateReceiverV1, DecodedTemplateReceiverV1,
    IndexedOptionalTemplateReceiverV1, IndexedTemplateReceiverV1, OptionalTemplateReceiverV1,
    TemplateReceiverBuildError, TemplateReceiverIndexError, TemplateReceiverResolutionError,
    TemplateReceiverSemanticValidationError, TemplateReceiverV1,
};
pub use roots::{
    DecodedPersistentLexicalRootV1, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticAuthority, DefaultTemplateRootSemanticValidationError,
    PersistentLexicalRootBuildError, PersistentLexicalRootResolver, PersistentLexicalRootV1,
};
pub use value_parameters::{
    CanonicalTemplateValueParametersV1, DecodedCanonicalTemplateValueParametersV1,
    DecodedTemplateValueParameterV1, IndexedCanonicalTemplateValueParametersV1,
    IndexedTemplateValueParameterV1, TemplateValueParameterBuildError,
    TemplateValueParameterIndexError, TemplateValueParameterListBuildError,
    TemplateValueParameterListIndexError, TemplateValueParameterListValidationError,
    TemplateValueParameterResolutionError, TemplateValueParameterSemanticValidationError,
    TemplateValueParameterV1,
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
