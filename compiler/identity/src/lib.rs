//! Persistent semantic identities shared across compiler stages and `.slib`.
//!
//! The public identity types are intentionally distinct even though every one
//! has the same 32-byte wire representation. Raw decoded bytes remain inside
//! [`DecodedPersistentId`] until a validator compares them with an identity
//! recomputed from the corresponding canonical key.
//!
//! ```compile_fail
//! use scoop_identity::{ConeCoordinate, PersistentTypeId};
//!
//! let cone = ConeCoordinate::reserved_core().identity().unwrap();
//! let _: PersistentTypeId = cone;
//! ```

mod capability;
mod cone;
mod entity;
mod ids;
mod source;
mod syntax;

pub use capability::{
    ArtifactCapabilityProfileId, BackendProfileWireId, CapabilityId, CapabilityIdError,
    CapabilityLabelError, ObjectFormatId, TargetProfileWireId,
};
pub use cone::{
    ConeCoordinate, ConeCoordinateComponent, ConeCoordinateError, ConeCoordinateTextError,
    ConeIdentity, DecodedConeCoordinate,
};
pub use entity::{
    AccessorRole, BindableEntity, BindingNamespace, BindingRole, BindingTarget, BindingTargetError,
    CallableApplicationKey, CallableArguments, CallableBodyKey, CallableBodyKeyKind,
    CallableInstantiationOwner, CallableMaterialization, CallableMaterializationContext,
    CallableTemplateOrigin, CallableTemplateOwner, CallbackApplicationIdentityError,
    CallbackApplicationKey, CallbackParameterIndex, CallbackRegistrationKey, ExportBindingKey,
    LocalBindingKey, LocalBindingRole, MainCallableBodyId, PropertyAccessorKey,
    SignatureCallableShape, StrongCallableDefinitionOwner,
};
pub use entity::{
    CDataPointee, CLayoutByteAlignment, CLayoutOverride, CPointerStorage, CanonicalCAbiError,
    CanonicalCAbiFunctionSignature, CanonicalCAbiLayout, CanonicalCAbiLayoutField,
    CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiParameter, CanonicalCAbiReturn,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType, IntegerBitWidth, Signedness,
    TargetCallingConvention,
};
pub use entity::{
    CallableAdapterEnvironmentKey, ClosureEnvironmentRole, ContinuationShellRole,
    CoroutineAdapterRole, EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityError,
    EnumVariantIdentityKey, ExactCallableSignature, FieldIdentityError, FieldIdentityKey,
    GeneratedCallableIdentityError, GeneratedCallableKey, GeneratedEnumVariantRole,
    GeneratedFieldKey, GeneratedNominalIdentityError, GeneratedNominalKey,
    InitializationCallableRole, LexicalCallableParent, LexicalCallableRole, LexicalParentError,
    OptionalExactOwner, SourceFieldKey,
};
pub use entity::{
    CallableOdrMemberId, OdrMemberDiscriminator, OdrMemberIdentityError, OdrMemberKey,
    OdrMemberRole, SpecializationKey,
};
pub use entity::{
    CallableOwner, CallingConvention, CanonicalExactTypeDiagnosticName, ConcreteExpressionOrigin,
    DeclarationName, DeclarationScope, DefinitionOrigin, DefinitionOriginRecord,
    DefinitionOriginSubject, DefinitionOwnerAtom, DefinitionOwnerChain, DispatchDeclarationOwner,
    DispatchRole, DispatchSlotKey, DispatchTableKey, DispatchTableRole, DuplicateSignatureKey,
    Effect, EvaluationOrigin, ExactTypeDiagnosticError, ExactTypeDiagnosticGraph, ExactTypeKey,
    ExpressionOrigin, NominalDeclarationOwner, NominalOwner, NonEmptyVec, NonEmptyVecError,
    OptionalExactInterface, OptionalSignatureType, PropertyOwner, SignatureTypeKey,
    SourceContextKey, SourceDeclarationIdentityError, SourceDeclarationKey,
    SourceDeclarationKeyError, SourceDeclarationKind, SourceDeclarationSite, SourceNominalKind,
    SourceOriginError, SourceSpan, SourceSpanError, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
pub use entity::{
    CallbackMode, GcEffect, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceCallingConvention,
    SourceExternFunctionAbi, SourceNativeContractError, SourceNativeExternalContract,
    SourceNativeExternalContractKey, SourceNativeExternalContractRecord, SourceNativeExternalOwner,
    SourceNativeLibraryBinding, SourceScoopAbiFunctionSignature,
};
pub use entity::{
    CanonicalNativeGroupName, CanonicalNativeLibraryName, CanonicalNativeNameError,
    NativeExternAbi, NativeExternalContract, NativeExternalContractFingerprintInput,
    NativeExternalContractRecord, NativeExternalSymbolKey, NativeLibraryBinding,
    NativeLibraryGrouping, NativeLibraryKind, NativeLinkRequirementKey, NativeLinkSymbol,
    NativeLinkSymbolError, SourceNativeSymbol, SourceNativeSymbolError,
};
pub use entity::{
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, ScoopAbiArgument, ScoopAbiError,
    ScoopAbiReturn, ScoopAbiValueShape,
};
pub use entity::{
    DefinitionOwner, ImmortalObjectKey, ImmortalObjectOwner, ImmortalObjectRole, LayoutKey,
    RepresentationRole, RuntimeIdentityError, ScanKey, ScanRole, StaticStorageKey, StorageRole,
};
pub use entity::{DerivedIdError, RuntimeTypeId, SafepointId, SafepointSiteKey, SafepointSiteRole};
pub use entity::{InitializationUnitKey, LocalValueKey, LocalValueSelector, SyntheticLocalRole};
pub use ids::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, DecodedPersistentId,
    GeneratedBridgeAtomId, GeneratedBridgeUnitId, NativeExternalContractFingerprint,
    NativeLinkRequirementId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, OdrGroupId,
    OdrMemberId, PersistentCallableApplicationId, PersistentCallableBodyId,
    PersistentCallbackApplicationId, PersistentCallbackRegistrationId, PersistentConstructorId,
    PersistentDispatchSlotId, PersistentDispatchTableId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentExportBindingId,
    PersistentExtensionPropertyId, PersistentFieldId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentId, PersistentIdMismatch, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentLocalBindingId, PersistentLocalValueId,
    PersistentNativeExternalSymbolId, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentSafepointSiteId, PersistentScanId, PersistentSourceContextId,
    PersistentSourceNativeExternalContractId, PersistentStaticStorageId, PersistentTypeAliasId,
    PersistentTypeId,
};
pub use source::{
    DecodedNormalizedSourcePath, DecodedSourceIdentity, NormalizedSourcePath,
    NormalizedSourcePathError, SemanticSourceNameError, SourceIdentity, SourceIdentityDecodeError,
    SourceIdentityError,
};
pub use syntax::{
    CanonicalIdentifier, CanonicalIdentifierError, DecodedCanonicalIdentifier, DecodedPackagePath,
    PackagePath,
};
