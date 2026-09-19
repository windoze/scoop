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
mod mangling;
mod record;
mod source;
mod syntax;
mod validation;

pub use capability::{
    ArtifactCapabilityProfileId, ArtifactCapabilityProfileRefinementError, BackendProfileWireId,
    CBridgeToolchainProfileId, CapabilityId, CapabilityIdError, CapabilityLabelError,
    CapabilityRefinementError, DecodedCapabilityId, ObjectFormatId, TargetProfileWireId,
};
pub use cone::{
    ConeCoordinate, ConeCoordinateComponent, ConeCoordinateError, ConeCoordinateTextError,
    ConeIdentity, DecodedConeCoordinate, RequestedConeKind,
};
pub use entity::{
    AccessorRole, BindableEntity, BindingIdentityResolutionError, BindingNamespace,
    BindingResolver, BindingRole, BindingTarget, BindingTargetError, CallableApplicationKey,
    CallableApplicationResolutionError, CallableArguments, CallableBodyKey, CallableBodyKeyKind,
    CallableBodyResolutionError, CallableInstantiationOwner, CallableMaterialization,
    CallableMaterializationContext, CallableTemplateOrigin, CallableTemplateOwner,
    CallbackApplicationIdentityError, CallbackApplicationKey, CallbackIdentityResolutionError,
    CallbackParameterIndex, CallbackRegistrationKey, CoreBuiltinNominal, CoreImportedCallableKind,
    CoreNativeBoundaryNominal, DecodedBindableEntity, DecodedCallableApplicationKey,
    DecodedCallableArguments, DecodedCallableBodyKey, DecodedCallableBodyKeyKind,
    DecodedCallableInstantiationOwner, DecodedCallableMaterialization,
    DecodedCallableMaterializationContext, DecodedCallableTemplateOrigin,
    DecodedCallableTemplateOwner, DecodedCallbackApplicationKey, DecodedCallbackRegistrationKey,
    DecodedDependencyCallableDeclarationId, DecodedExecutableSourceEntryIdentity,
    DecodedExportBindingKey, DecodedLocalBindingKey, DecodedPropertyAccessorKey,
    DecodedSignatureCallableShape, DecodedStrongCallableDefinitionOwner,
    DependencyCallableDeclarationId, ExecutableSourceEntryIdentity,
    ExecutableSourceEntryIdentityError, ExportBindingKey, LocalBindingKey, LocalBindingRole,
    MainCallableBodyId, PropertyAccessorKey, SignatureCallableShape, StrongCallableDefinitionOwner,
};
pub use entity::{
    CDataPointee, CLayoutByteAlignment, CLayoutOverride, CPointerStorage, CanonicalCAbiError,
    CanonicalCAbiFunctionSignature, CanonicalCAbiLayout, CanonicalCAbiLayoutField,
    CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiParameter, CanonicalCAbiResolutionError,
    CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType,
    DecodedCDataPointee, DecodedCLayoutOverride, DecodedCPointerStorage,
    DecodedCanonicalCAbiFunctionSignature, DecodedCanonicalCAbiLayout,
    DecodedCanonicalCAbiLayoutField, DecodedCanonicalCAbiLayoutFingerprintRecord,
    DecodedCanonicalCAbiParameter, DecodedCanonicalCAbiReturn,
    DecodedCanonicalCAbiSignatureFingerprintRecord, DecodedCanonicalCStorageType, IntegerBitWidth,
    Signedness, TargetCallingConvention,
};
pub use entity::{
    CallableAdapterEnvironmentKey, ClosureEnvironmentRole, ContinuationShellRole,
    CoroutineAdapterRole, DecodedCallableAdapterEnvironmentKey, DecodedEnumVariantFieldKey,
    DecodedEnumVariantFieldSelector, DecodedEnumVariantIdentityKey, DecodedExactCallableSignature,
    DecodedGeneratedCallableKey, DecodedGeneratedNominalKey, DecodedLexicalCallableParent,
    DecodedOptionalExactOwner, EnumVariantFieldKey, EnumVariantFieldResolutionError,
    EnumVariantFieldSelector, EnumVariantIdentityError, EnumVariantIdentityKey,
    EnumVariantResolutionError, ExactCallableSignature, ExactCallableSignatureResolutionError,
    ExactOrdinaryNoArgUnitSignature, FieldIdentityError, FieldIdentityKey,
    FieldIdentityResolutionError, FieldIdentityView, GeneratedCallableIdentityError,
    GeneratedCallableKey, GeneratedCallableResolutionError, GeneratedEnumVariantRole,
    GeneratedFieldKey, GeneratedNominalIdentityError, GeneratedNominalKey,
    GeneratedNominalResolutionError, InitializationCallableRole, LexicalCallableParent,
    LexicalCallableRole, LexicalParentError, MeteredExactCallableSignatureResolutionError,
    OptionalExactOwner, SourceFieldKey, StaticNoGcCallbackStorageBridgeId,
    StaticNoGcCallbackStorageBridgeIdentityError,
};
pub use entity::{
    CallableOdrMemberId, DecodedOdrMemberDiscriminator, DecodedOdrMemberKey,
    DecodedSpecializationKey, OdrIdentityResolutionError, OdrMemberDiscriminator,
    OdrMemberIdentityError, OdrMemberKey, OdrMemberResolver, OdrMemberRole, SpecializationKey,
    SpecializationResolver,
};
pub use entity::{
    CallableOwner, CallingConvention, CanonicalExactTypeDiagnosticName, ConcreteExpressionOrigin,
    DeclarationName, DeclarationScope, DecodedCallableOwner, DecodedConcreteExpressionOrigin,
    DecodedDeclarationName, DecodedDeclarationScope, DecodedDefinitionOrigin,
    DecodedDefinitionOriginRecord, DecodedDefinitionOriginSubject, DecodedDefinitionOwnerAtom,
    DecodedDefinitionOwnerChain, DecodedDispatchDeclarationOwner, DecodedDispatchSlotKey,
    DecodedDispatchTableKey, DecodedDuplicateSignatureKey, DecodedEvaluationOrigin,
    DecodedExactTypeKey, DecodedExpressionOrigin, DecodedFieldIdentityKey,
    DecodedGeneratedFieldKey, DecodedNominalDeclarationOwner, DecodedNominalOwner,
    DecodedOptionalExactInterface, DecodedOptionalSignatureType, DecodedPropertyOwner,
    DecodedSignatureTypeKey, DecodedSourceContextKey, DecodedSourceDeclarationKey,
    DecodedSourceFieldKey, DecodedSourceSpan, DefinitionOrigin, DefinitionOriginRecord,
    DefinitionOriginRecordResolutionError, DefinitionOriginSubject,
    DefinitionOriginSubjectResolver, DefinitionOwnerAtom, DefinitionOwnerChain,
    DefinitionOwnerResolutionError, DispatchDeclarationOwner, DispatchIdentityResolutionError,
    DispatchRole, DispatchSlotKey, DispatchTableKey, DispatchTableRole, DuplicateSignatureKey,
    Effect, EvaluationOrigin, ExactTypeDiagnosticError, ExactTypeDiagnosticGraph, ExactTypeKey,
    ExactTypeResolutionError, ExpressionOrigin, NominalDeclarationOwner, NominalOwner, NonEmptyVec,
    NonEmptyVecError, OptionalExactInterface, OptionalSignatureType, PropertyOwner,
    SignatureTypeKey, SourceContextKey, SourceContextResolutionError, SourceContextResolver,
    SourceDeclarationIdentityError, SourceDeclarationKey, SourceDeclarationKeyError,
    SourceDeclarationKind, SourceDeclarationResolutionError, SourceDeclarationSite,
    SourceNominalKind, SourceOriginError, SourceOriginResolutionError, SourceSpan, SourceSpanError,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};
pub use entity::{
    CallbackMode, DecodedSourceCAbiFunctionSignature, DecodedSourceCAbiReturn,
    DecodedSourceExternFunctionAbi, DecodedSourceNativeExternalContract,
    DecodedSourceNativeExternalContractKey, DecodedSourceNativeExternalContractRecord,
    DecodedSourceNativeExternalOwner, DecodedSourceNativeLibraryBinding,
    DecodedSourceScoopAbiFunctionSignature, GcEffect, SourceCAbiFunctionSignature,
    SourceCAbiReturn, SourceCallingConvention, SourceExternFunctionAbi, SourceNativeContractError,
    SourceNativeExternalContract, SourceNativeExternalContractKey,
    SourceNativeExternalContractRecord, SourceNativeExternalOwner,
    SourceNativeExternalResolutionError, SourceNativeLibraryBinding,
    SourceScoopAbiFunctionSignature,
};
pub use entity::{
    CanonicalNativeGroupName, CanonicalNativeLibraryName, CanonicalNativeNameError,
    DecodedCanonicalNativeGroupName, DecodedCanonicalNativeLibraryName, DecodedNativeExternAbi,
    DecodedNativeExternalContract, DecodedNativeExternalContractRecord,
    DecodedNativeExternalSymbolKey, DecodedNativeLibraryBinding, DecodedNativeLibraryGrouping,
    DecodedNativeLinkRequirementKey, DecodedNativeLinkSymbol, DecodedSourceNativeSymbol,
    NativeExternAbi, NativeExternalContract, NativeExternalContractFingerprintError,
    NativeExternalContractFingerprintInput, NativeExternalContractRecord,
    NativeExternalContractResolutionError, NativeExternalSymbolKey, NativeLibraryBinding,
    NativeLibraryGrouping, NativeLibraryKind, NativeLinkRequirementKey, NativeLinkSymbol,
    NativeLinkSymbolError, NativeLinkValidationError, SourceNativeSymbol, SourceNativeSymbolError,
};
pub use entity::{
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage,
    DecodedCanonicalScoopAbiFunctionSignature, DecodedCanonicalScoopStorage,
    DecodedScoopAbiArgument, DecodedScoopAbiReturn, ScoopAbiArgument, ScoopAbiError,
    ScoopAbiResolutionError, ScoopAbiReturn, ScoopAbiValueShape,
};
pub use entity::{
    ConeImageSupportRole, DecodedDefinitionAtomSubkey, DecodedObjectDefinitionAtomKey,
    DecodedObjectDefinitionPlanKey, DecodedObjectDefinitionPlanOwner,
    DecodedStrongDefinitionEntity, DefinitionAtomResolver, DefinitionAtomRole,
    DefinitionAtomSubkey, ObjectDefinitionAtomKey, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanKey, ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole,
    ObjectDefinitionResolutionError, StrongDefinitionEntity, StrongDefinitionEntityKind,
    StrongDefinitionResolver, StrongDefinitionRole,
};
pub use entity::{
    DecodedDigestNodeKey, DecodedDigestOwnerAndRoleKey, DecodedDigestPatchIntentKey, DigestKind,
    DigestNodeKey, DigestNodeKeyError, DigestNodeKeyResolutionError, DigestOwnerAndRoleKey,
    DigestOwnerResolver, DigestPatchIntentKey, DigestPatchIntentResolutionError,
    DigestPatchIntentResolver, DigestSemanticFieldRole,
};
pub use entity::{
    DecodedGeneratedBridgeAtomKey, DecodedGeneratedBridgeAtomRoleKey,
    DecodedGeneratedBridgeSemanticTarget, DecodedGeneratedBridgeUnitKey, GeneratedBridgeAtomKey,
    GeneratedBridgeAtomRoleKey, GeneratedBridgeSemanticTarget, GeneratedBridgeUnitKey,
    GeneratedBridgeUnitResolutionError,
};
pub use entity::{
    DecodedImmortalObjectKey, DecodedLayoutKey, DecodedScanKey, DecodedStaticStorageKey,
    DefinitionOwner, ImmortalObjectKey, ImmortalObjectOwner, ImmortalObjectRole, LayoutKey,
    LayoutKeyResolutionError, RepresentationRole, RuntimeIdentityError, ScanKey, ScanRole,
    StaticStorageKey, StaticStorageResolutionError, StorageRole,
};
pub use entity::{
    DecodedInitializationUnitKey, DecodedLocalValueKey, InitializationUnitKey,
    InitializationUnitResolutionError, LocalValueKey, LocalValueResolutionError,
    LocalValueSelector, SyntheticLocalRole,
};
pub use entity::{
    DecodedSafepointSiteKey, DerivedIdError, RuntimeTypeId, SafepointId, SafepointSiteKey,
    SafepointSiteRole,
};
pub use ids::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, DecodedPersistentId,
    DigestNodeId, DigestPatchIntentId, GeneratedBridgeAtomId, GeneratedBridgeUnitId,
    NativeExternalContractFingerprint, NativeLinkRequirementId, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, OdrGroupId, OdrMemberId, PersistentCallableApplicationId,
    PersistentCallableBodyId, PersistentCallbackApplicationId, PersistentCallbackRegistrationId,
    PersistentConstructorId, PersistentDispatchSlotId, PersistentDispatchTableId,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentExportBindingId, PersistentExtensionPropertyId, PersistentFieldId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentId, PersistentIdMismatch, PersistentIdResolver,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentLayoutId, PersistentLocalBindingId, PersistentLocalValueId,
    PersistentNativeExternalSymbolId, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentSafepointSiteId, PersistentScanId, PersistentSourceContextId,
    PersistentSourceNativeExternalContractId, PersistentStaticStorageId, PersistentTypeAliasId,
    PersistentTypeId, SourceSignatureFingerprint,
};
pub use mangling::{
    DecodedManglingSchemaIdentity, DecodedPersistentSymbolKey, DecodedPersistentSymbolRequest,
    DecodedPersistentSymbolRequestTable, LinkageClass, MangledSymbol, ManglingSchemaIdentity,
    ManglingSchemaIdentityError, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolKind,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentSymbolResolutionError,
    PersistentSymbolResolver,
};
pub use record::{
    CborIdentityKey, CborIdentityRecord, DecodedCborIdentityRecord, DecodedRuntimeIdentityRecord,
    IdentityRecordResolutionError, IdentityRecordValidationError, RuntimeIdentityKey,
    RuntimeIdentityRecord, RuntimeIdentityRecordBuildError, RuntimeIdentityRecordValidationError,
    StableIdentityOrderError, stable_topological_identity_delta_order,
    stable_topological_identity_order, validate_stable_topological_identity_order,
};
pub use source::{
    DecodedNormalizedSourcePath, DecodedSourceContentDigest, DecodedSourceIdentity,
    NormalizedSourcePath, NormalizedSourcePathError, SemanticSourceNameError, SourceContentDigest,
    SourceContentDigestError, SourceIdentity, SourceIdentityDecodeError, SourceIdentityError,
    SourceIdentityResolutionError,
};
pub use syntax::{
    CanonicalIdentifier, CanonicalIdentifierError, DecodedCanonicalIdentifier, DecodedPackagePath,
    PackagePath,
};
pub use validation::{
    DecodedIdentityKey, HirIdentityLayer, IdentityLayer, IdentityReferenceError,
    IdentityValidationError, ImportedIdentityId, ImportedIdentityLayer, ImportedIdentityLayers,
    ImportedIdentityMap, LirIdentityLayer, MirIdentityLayer, PendingIdentityValidation,
    SemanticIdentityImport, SemanticIdentityImportError, SemanticIdentitySession,
    SemanticOriginFingerprint, ValidatedIdentityGraph,
};
