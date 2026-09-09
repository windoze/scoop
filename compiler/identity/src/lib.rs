//! Persistent semantic identities shared across compiler stages and `.slib`.
//!
//! The public identity types are intentionally distinct even though every one
//! has the same 32-byte wire representation. Raw decoded bytes remain inside
//! [`DecodedPersistentIdV1`] until a validator compares them with an identity
//! recomputed from the corresponding canonical key.
//!
//! ```compile_fail
//! use scoop_identity::{ConeCoordinate, PersistentTypeId};
//!
//! let cone = ConeCoordinate::reserved_core().identity().unwrap();
//! let _: PersistentTypeId = cone;
//! ```

mod cone;
mod entity;
mod ids;
mod source;
mod syntax;

pub use cone::{
    ConeCoordinate, ConeCoordinateComponent, ConeCoordinateError, ConeCoordinateTextError,
    ConeIdentity, DecodedConeCoordinateV1,
};
pub use entity::{
    AccessorRoleV1, BindableEntityV1, BindingNamespaceV1, BindingRoleV1, BindingTargetError,
    BindingTargetV1, CallableApplicationKeyV1, CallableArgumentsV1, CallableInstantiationOwnerV1,
    CallableMaterializationContextV1, CallableMaterializationV1, CallableTemplateOriginV1,
    CallableTemplateOwnerV1, ExportBindingKeyV1, LocalBindingKeyV1, LocalBindingRoleV1,
    PropertyAccessorKeyV1,
};
pub use entity::{
    CallableAdapterEnvironmentKeyV1, ClosureEnvironmentRoleV1, ContinuationShellRoleV1,
    CoroutineAdapterRoleV1, EnumVariantFieldKeyV1, EnumVariantFieldSelectorV1,
    EnumVariantIdentityError, EnumVariantIdentityKeyV1, ExactCallableSignatureV1,
    FieldIdentityError, FieldIdentityKeyV1, GeneratedCallableIdentityError, GeneratedCallableKeyV1,
    GeneratedEnumVariantRoleV1, GeneratedFieldKeyV1, GeneratedNominalIdentityError,
    GeneratedNominalKeyV1, InitializationCallableRoleV1, LexicalCallableParentV1,
    LexicalCallableRoleV1, LexicalParentError, OptionalExactOwnerV1, SourceFieldKeyV1,
};
pub use entity::{
    CallableOwnerV1, CallingConventionV1, CanonicalExactTypeDiagnosticName,
    ConcreteExpressionOriginV1, DeclarationNameV1, DeclarationScopeV1, DefinitionOriginRecordV1,
    DefinitionOriginSubjectV1, DefinitionOriginV1, DefinitionOwnerAtomV1, DefinitionOwnerChainV1,
    DispatchDeclarationOwnerV1, DispatchRoleV1, DispatchSlotKeyV1, DispatchTableKeyV1,
    DispatchTableRoleV1, DuplicateSignatureKeyV1, EffectV1, EvaluationOriginV1,
    ExactTypeDiagnosticError, ExactTypeDiagnosticGraphV1, ExactTypeKeyV1, ExpressionOriginV1,
    NominalDeclarationOwnerV1, NominalOwnerV1, NonEmptyVec, NonEmptyVecError,
    OptionalExactInterfaceV1, OptionalSignatureTypeV1, PropertyOwnerV1, SignatureTypeKeyV1,
    SourceContextKeyV1, SourceDeclarationIdentityError, SourceDeclarationKeyError,
    SourceDeclarationKeyV1, SourceDeclarationKindV1, SourceDeclarationSiteV1, SourceNominalKindV1,
    SourceOriginError, SourceSpanError, SourceSpanV1, StructuralDefinitionPathV1,
    StructuralDefinitionSiteRoleV1, StructuralPathSegmentV1,
};
pub use entity::{
    InitializationUnitKeyV1, LocalValueKeyV1, LocalValueSelectorV1, SyntheticLocalRoleV1,
};
pub use ids::{
    DecodedPersistentIdV1, GeneratedBridgeAtomId, GeneratedBridgeUnitId, NativeLinkRequirementId,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId, OdrGroupId, OdrMemberId,
    PersistentCallableApplicationId, PersistentCallableBodyId, PersistentCallbackApplicationId,
    PersistentCallbackRegistrationId, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentExtensionPropertyId,
    PersistentFieldId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentIdMismatch, PersistentIdV1,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentLayoutId,
    PersistentLocalBindingId, PersistentLocalValueId, PersistentNativeExternalSymbolId,
    PersistentObjectValueId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentSafepointSiteId, PersistentScanId, PersistentSourceContextId,
    PersistentSourceNativeExternalContractId, PersistentStaticStorageId, PersistentTypeAliasId,
    PersistentTypeId,
};
pub use source::{
    DecodedNormalizedSourcePathV1, DecodedSourceIdentityV1, NormalizedSourcePath,
    NormalizedSourcePathError, SemanticSourceNameError, SourceIdentity, SourceIdentityDecodeError,
    SourceIdentityError,
};
pub use syntax::{
    CanonicalIdentifier, CanonicalIdentifierError, DecodedCanonicalIdentifierV1,
    DecodedPackagePathV1, PackagePath,
};
