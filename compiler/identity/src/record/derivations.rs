use scoop_wire::HashError;

use super::{CborIdentityKey, private};
use crate::{
    CallableApplicationKey, CallbackApplicationKey, CallbackRegistrationKey,
    CanonicalCAbiFunctionSignature, CanonicalCAbiLayout, CanonicalCAbiLayoutFingerprint,
    CanonicalCAbiSignatureFingerprint, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DispatchSlotKey, DispatchTableKey, EnumVariantFieldKey,
    EnumVariantIdentityError, EnumVariantIdentityKey, ExactTypeKey, ExportBindingKey,
    FieldIdentityError, FieldIdentityKey, GeneratedBridgeAtomId, GeneratedBridgeAtomKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, GeneratedCallableIdentityError,
    GeneratedCallableKey, GeneratedNominalIdentityError, GeneratedNominalKey, ImmortalObjectKey,
    InitializationUnitKey, LayoutKey, LocalBindingKey, LocalValueKey,
    NativeExternalContractFingerprint, NativeExternalContractFingerprintInput,
    NativeExternalSymbolKey, NativeLinkRequirementId, NativeLinkRequirementKey,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, OdrGroupId, OdrMemberId, OdrMemberKey,
    PersistentCallableApplicationId, PersistentCallbackApplicationId,
    PersistentCallbackRegistrationId, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentExtensionPropertyId,
    PersistentFieldId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentLayoutId, PersistentLocalBindingId,
    PersistentLocalValueId, PersistentNativeExternalSymbolId, PersistentObjectValueId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSafepointSiteId,
    PersistentScanId, PersistentSourceContextId, PersistentStaticStorageId, PersistentTypeAliasId,
    PersistentTypeId, PropertyAccessorKey, SafepointSiteKey, ScanKey, SourceContextKey,
    SourceDeclarationIdentityError, SourceDeclarationKey, SourceNativeExternalContractKey,
    SpecializationKey, StaticStorageKey,
};

macro_rules! impl_cbor_identity_key {
    ($key:ty => $id:ty, $error:ty, $derive:path) => {
        impl private::CborIdentityKey<$id> for $key {}

        impl CborIdentityKey<$id> for $key {
            type Error = $error;

            fn derive_identity(&self) -> Result<$id, Self::Error> {
                $derive(self)
            }
        }
    };
}

macro_rules! impl_hash_identity_key {
    ($key:ty => $id:ty, $derive:path) => {
        impl_cbor_identity_key!($key => $id, HashError, $derive);
    };
}

impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentTypeId,
    SourceDeclarationIdentityError,
    PersistentTypeId::from_source_declaration
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentGenericTypeId,
    SourceDeclarationIdentityError,
    PersistentGenericTypeId::from_source_declaration
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentFunctionId,
    SourceDeclarationIdentityError,
    PersistentFunctionId::from_source_declaration
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentGenericFunctionId,
    SourceDeclarationIdentityError,
    PersistentGenericFunctionId::from_source_declaration
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentConstructorId,
    SourceDeclarationIdentityError,
    PersistentConstructorId::from_source_declaration
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentPropertyId,
    SourceDeclarationIdentityError,
    PersistentPropertyId::from_source_declaration
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentExtensionPropertyId,
    SourceDeclarationIdentityError,
    PersistentExtensionPropertyId::from_source_declaration
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentObjectValueId,
    SourceDeclarationIdentityError,
    PersistentObjectValueId::from_source_object
);
impl_cbor_identity_key!(
    SourceDeclarationKey => PersistentTypeAliasId,
    SourceDeclarationIdentityError,
    PersistentTypeAliasId::from_source_declaration
);
impl_hash_identity_key!(
    PropertyAccessorKey => PersistentPropertyAccessorId,
    PersistentPropertyAccessorId::from_key
);
impl_cbor_identity_key!(
    FieldIdentityKey => PersistentFieldId,
    FieldIdentityError,
    PersistentFieldId::from_key
);
impl_cbor_identity_key!(
    EnumVariantIdentityKey => PersistentEnumVariantId,
    EnumVariantIdentityError,
    PersistentEnumVariantId::from_key
);
impl_hash_identity_key!(
    EnumVariantFieldKey => PersistentEnumVariantFieldId,
    PersistentEnumVariantFieldId::from_key
);
impl_hash_identity_key!(ExactTypeKey => PersistentExactTypeId, PersistentExactTypeId::from_key);
impl_hash_identity_key!(
    ExportBindingKey => PersistentExportBindingId,
    PersistentExportBindingId::from_key
);
impl_hash_identity_key!(
    CallableApplicationKey => PersistentCallableApplicationId,
    PersistentCallableApplicationId::from_key
);
impl_cbor_identity_key!(
    GeneratedCallableKey => PersistentGeneratedCallableId,
    GeneratedCallableIdentityError,
    PersistentGeneratedCallableId::from_key
);
impl_cbor_identity_key!(
    GeneratedNominalKey => PersistentTypeId,
    GeneratedNominalIdentityError,
    PersistentTypeId::from_generated_key
);
impl_hash_identity_key!(
    DispatchSlotKey => PersistentDispatchSlotId,
    PersistentDispatchSlotId::from_key
);
impl_hash_identity_key!(
    InitializationUnitKey => PersistentInitializationUnitId,
    PersistentInitializationUnitId::from_key
);
impl_hash_identity_key!(
    SourceContextKey => PersistentSourceContextId,
    PersistentSourceContextId::from_key
);
impl_hash_identity_key!(
    LocalBindingKey => PersistentLocalBindingId,
    PersistentLocalBindingId::from_key
);
impl_hash_identity_key!(LocalValueKey => PersistentLocalValueId, PersistentLocalValueId::from_key);
impl_hash_identity_key!(
    CallbackRegistrationKey => PersistentCallbackRegistrationId,
    PersistentCallbackRegistrationId::from_key
);
impl_hash_identity_key!(
    CallbackApplicationKey => PersistentCallbackApplicationId,
    PersistentCallbackApplicationId::from_key
);
impl_hash_identity_key!(SpecializationKey => OdrGroupId, OdrGroupId::from_key);
impl_hash_identity_key!(OdrMemberKey => OdrMemberId, OdrMemberId::from_key);
impl_hash_identity_key!(LayoutKey => PersistentLayoutId, PersistentLayoutId::from_key);
impl_hash_identity_key!(ScanKey => PersistentScanId, PersistentScanId::from_key);
impl_hash_identity_key!(
    DispatchTableKey => PersistentDispatchTableId,
    PersistentDispatchTableId::from_key
);
impl_hash_identity_key!(
    StaticStorageKey => PersistentStaticStorageId,
    PersistentStaticStorageId::from_key
);
impl_hash_identity_key!(
    ImmortalObjectKey => PersistentImmortalObjectId,
    PersistentImmortalObjectId::from_key
);
impl_hash_identity_key!(
    SafepointSiteKey => PersistentSafepointSiteId,
    PersistentSafepointSiteId::from_key
);
impl_hash_identity_key!(
    SourceNativeExternalContractKey => crate::PersistentSourceNativeExternalContractId,
    crate::PersistentSourceNativeExternalContractId::from_key
);
impl_hash_identity_key!(
    NativeExternalSymbolKey => PersistentNativeExternalSymbolId,
    PersistentNativeExternalSymbolId::from_key
);
impl_hash_identity_key!(
    NativeExternalContractFingerprintInput => NativeExternalContractFingerprint,
    NativeExternalContractFingerprint::from_input
);
impl_hash_identity_key!(
    CanonicalCAbiFunctionSignature => CanonicalCAbiSignatureFingerprint,
    CanonicalCAbiSignatureFingerprint::from_signature
);
impl_hash_identity_key!(
    CanonicalCAbiLayout => CanonicalCAbiLayoutFingerprint,
    CanonicalCAbiLayoutFingerprint::from_layout
);
impl_hash_identity_key!(
    GeneratedBridgeUnitKey => GeneratedBridgeUnitId,
    GeneratedBridgeUnitId::from_key
);
impl_hash_identity_key!(
    GeneratedBridgeAtomKey => GeneratedBridgeAtomId,
    GeneratedBridgeAtomId::from_key
);
impl_hash_identity_key!(
    NativeLinkRequirementKey => NativeLinkRequirementId,
    NativeLinkRequirementId::from_key
);
impl_hash_identity_key!(
    ObjectDefinitionPlanKey => ObjectDefinitionPlanId,
    ObjectDefinitionPlanId::from_key
);
impl_hash_identity_key!(
    ObjectDefinitionAtomKey => ObjectDefinitionAtomId,
    ObjectDefinitionAtomId::from_key
);
impl_hash_identity_key!(DigestNodeKey => DigestNodeId, DigestNodeId::from_key);
impl_hash_identity_key!(
    DigestPatchIntentKey => DigestPatchIntentId,
    DigestPatchIntentId::from_key
);
