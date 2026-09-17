use scoop_identity::{CallableTemplateOrigin, DecodedCallableTemplateOrigin};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::CallableDeclarationIdResolver;

mod binder_uses;
mod body;
mod locals;
mod receiver;
mod references;
mod roots;
mod table;
mod template;
mod value_parameters;

pub use binder_uses::{
    BinderUseListBuildError, BinderUseListSemanticValidationError, BinderUseListValidationError,
    CanonicalBinderUseListV1, DecodedCanonicalBinderUseListV1,
};
pub use body::{
    DecodedDefaultAnonymousFunctionV1, DecodedDefaultAppliedOptionV1,
    DecodedDefaultArrayAssemblyPartV1, DecodedDefaultArrayAssemblyV1, DecodedDefaultAssignTargetV1,
    DecodedDefaultBindingActionV1, DecodedDefaultBindingClassComponentV1,
    DecodedDefaultBindingLeafV1, DecodedDefaultBindingPlanV1, DecodedDefaultBindingProjectionV1,
    DecodedDefaultBindingShapeV1, DecodedDefaultBindingStructFieldV1,
    DecodedDefaultBindingTemporaryV1, DecodedDefaultBoundCallableRefV1,
    DecodedDefaultBoundCallableSourceV1, DecodedDefaultCallableBodyTypeArgumentsV1,
    DecodedDefaultCallableDeclarationV1, DecodedDefaultCallableRefV1,
    DecodedDefaultCallableReferenceTargetV1, DecodedDefaultCallableReferenceV1,
    DecodedDefaultCaptureV1, DecodedDefaultCatchV1, DecodedDefaultClassConstructorIdV1,
    DecodedDefaultConstructorRefV1, DecodedDefaultEnumVariantFieldRefV1,
    DecodedDefaultEnumVariantRefV1, DecodedDefaultExpressionV1, DecodedDefaultFieldRefV1,
    DecodedDefaultForIterationPlanV1, DecodedDefaultIntegerArgumentsV1,
    DecodedDefaultIntegerOperationV1, DecodedDefaultIteratorConformanceV1,
    DecodedDefaultIteratorNextV1, DecodedDefaultLambdaV1, DecodedDefaultLiteralEqualityV1,
    DecodedDefaultLocalFunctionV1, DecodedDefaultMethodCalleeV1, DecodedDefaultPatternFieldV1,
    DecodedDefaultPatternV1, DecodedDefaultPlaceV1, DecodedDefaultStatementV1,
    DecodedDefaultStringOwnerV1, DecodedDefaultTryV1, DecodedDefaultWhenArmV1,
    DecodedDefaultWhenFallbackV1, DecodedDefaultWhenGuardV1, DecodedDefaultWhenV1,
    DecodedExportDefaultBodyV1, DecodedOptionalDefaultExpressionV1,
    DecodedOptionalDefaultStatementListV1, DecodedOptionalDefaultWhenGuardV1,
    DefaultAnonymousFunctionV1, DefaultAppliedOptionV1, DefaultArrayAccessKindV1,
    DefaultArrayAssemblyBuildError, DefaultArrayAssemblyPartV1, DefaultArrayAssemblyV1,
    DefaultAssignTargetIndexError, DefaultAssignTargetResolutionError, DefaultAssignTargetV1,
    DefaultBinaryOperatorV1, DefaultBinderRefV1, DefaultBindingActionBuildError,
    DefaultBindingActionV1, DefaultBindingActionViewV1, DefaultBindingClassComponentV1,
    DefaultBindingLeafIndexError, DefaultBindingLeafResolutionError, DefaultBindingLeafV1,
    DefaultBindingPlanBuildError, DefaultBindingPlanV1, DefaultBindingProjectionBuildError,
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
    DefaultCaptureIndexError, DefaultCaptureResolutionError, DefaultCaptureV1, DefaultCatchV1,
    DefaultClassConstructorIdResolver, DefaultClassConstructorIdV1,
    DefaultConstructorRefResolutionError, DefaultConstructorRefV1,
    DefaultConstructorReferenceResolver, DefaultControlFlowBuildError,
    DefaultControlFlowIndexError, DefaultControlFlowResolutionError,
    DefaultEnumVariantFieldRefResolutionError, DefaultEnumVariantFieldRefV1,
    DefaultEnumVariantRefResolutionError, DefaultEnumVariantRefV1, DefaultExpressionBuildError,
    DefaultExpressionIndexError, DefaultExpressionKindV1, DefaultExpressionReferenceResolver,
    DefaultExpressionResolutionError, DefaultExpressionV1, DefaultFieldRefResolutionError,
    DefaultFieldRefV1, DefaultFieldReferenceResolver, DefaultForIterationPlanBuildError,
    DefaultForIterationPlanIndexError, DefaultForIterationPlanResolutionError,
    DefaultForIterationPlanV1, DefaultForeignCallbackOperationV1, DefaultIntegerArgumentsV1,
    DefaultIntegerDivRemV1, DefaultIntegerKindV1, DefaultIntegerOperationResolutionError,
    DefaultIntegerOperationV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
    DefaultLambdaV1, DefaultLexicalCallableBuildError, DefaultLexicalCallableIndexError,
    DefaultLexicalCallableResolutionError, DefaultLiteralEqualityResolutionError,
    DefaultLiteralEqualityV1, DefaultLocalFunctionBuildError, DefaultLocalFunctionIndexError,
    DefaultLocalFunctionResolutionError, DefaultLocalFunctionV1,
    DefaultMethodCalleeResolutionError, DefaultMethodCalleeV1,
    DefaultNestedCallableReferenceResolver, DefaultNoGcIntegerOperationV1,
    DefaultPatternBuildError, DefaultPatternFieldV1, DefaultPatternIndexError,
    DefaultPatternReferenceResolver, DefaultPatternResolutionError, DefaultPatternV1,
    DefaultPatternViewV1, DefaultPlaceIndexError, DefaultPlaceResolutionError, DefaultPlaceV1,
    DefaultPrimitiveBinaryKindV1, DefaultPrimitiveUnaryKindV1, DefaultStatementBuildError,
    DefaultStatementIndexError, DefaultStatementKindV1, DefaultStatementReferenceResolver,
    DefaultStatementResolutionError, DefaultStatementV1, DefaultStringOwnerResolutionError,
    DefaultStringOwnerV1, DefaultTryV1, DefaultUnaryOperatorV1, DefaultWhenArmV1,
    DefaultWhenFallbackV1, DefaultWhenFallbackViewV1, DefaultWhenGuardV1, DefaultWhenV1,
    ExportDefaultBodyBuildError, ExportDefaultBodyIndexError, ExportDefaultBodyResolutionError,
    ExportDefaultBodyV1, IndexedDefaultAnonymousFunctionV1, IndexedDefaultAppliedOptionV1,
    IndexedDefaultAssignTargetV1, IndexedDefaultBindingActionV1, IndexedDefaultBindingLeafV1,
    IndexedDefaultBindingPlanV1, IndexedDefaultBindingShapeV1, IndexedDefaultBindingTemporaryV1,
    IndexedDefaultCallableReferenceV1, IndexedDefaultCaptureV1, IndexedDefaultCatchV1,
    IndexedDefaultExpressionV1, IndexedDefaultForIterationPlanV1,
    IndexedDefaultIteratorConformanceV1, IndexedDefaultIteratorNextV1, IndexedDefaultLambdaV1,
    IndexedDefaultLocalFunctionV1, IndexedDefaultPatternV1, IndexedDefaultPlaceV1,
    IndexedDefaultStatementV1, IndexedDefaultTryV1, IndexedDefaultWhenArmV1,
    IndexedDefaultWhenFallbackV1, IndexedDefaultWhenGuardV1, IndexedDefaultWhenV1,
    IndexedExportDefaultBodyV1, IndexedOptionalDefaultStatementListV1,
    IndexedOptionalDefaultWhenGuardV1, OptionalDefaultExpressionV1, OptionalDefaultStatementListV1,
    OptionalDefaultStatementListViewV1, OptionalDefaultWhenGuardV1,
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
pub use references::{
    DecodedExportDefaultAccessWitnessV1, DecodedExportDefaultCallableReferenceV1,
    DecodedExportDefaultCallableTargetV1, DecodedExportDefaultConstructorReferenceV1,
    DecodedExportDefaultFieldReferenceV1, DecodedExportDefaultGlobalReferenceV1,
    DecodedExportDefaultReferenceSetV1, DecodedExportDefaultReferenceV1,
    DecodedExportDefaultSingletonReferenceV1, DecodedExportDefaultTypeReferenceV1,
    ExportDefaultAccessWitnessV1, ExportDefaultCallDomainV1, ExportDefaultCallableReferenceV1,
    ExportDefaultCallableTargetBuildError, ExportDefaultCallableTargetResolutionError,
    ExportDefaultCallableTargetV1, ExportDefaultConstructorReferenceV1,
    ExportDefaultFieldReferenceV1, ExportDefaultGlobalReferenceV1, ExportDefaultReferenceKindV1,
    ExportDefaultReferenceResolutionError, ExportDefaultReferenceResolver,
    ExportDefaultReferenceSetBuildError, ExportDefaultReferenceSetV1,
    ExportDefaultReferenceSetValidationError, ExportDefaultReferenceTargetResolutionError,
    ExportDefaultReferenceV1, ExportDefaultSingletonReferenceV1, ExportDefaultTargetDomainV1,
    ExportDefaultTypeReferenceV1,
};
pub use roots::{
    DecodedPersistentLexicalRootV1, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticAuthority, DefaultTemplateRootSemanticValidationError,
    PersistentLexicalRootBuildError, PersistentLexicalRootResolver, PersistentLexicalRootV1,
};
pub use table::{
    CanonicalExportDefaultTemplatesV1, DecodedCanonicalExportDefaultTemplatesV1,
    ExportDefaultTemplateLookupError, ExportDefaultTemplateSetBuildError,
    ExportDefaultTemplateSetIndexError, ExportDefaultTemplateSetValidationError,
    IndexedCanonicalExportDefaultTemplatesV1,
};
pub use template::{
    DecodedExportDefaultTemplateV1, ExportDefaultTemplateBuildError,
    ExportDefaultTemplateIndexError, ExportDefaultTemplateLocalIndexError,
    ExportDefaultTemplateResolutionError, ExportDefaultTemplateV1, IndexedExportDefaultTemplateV1,
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
