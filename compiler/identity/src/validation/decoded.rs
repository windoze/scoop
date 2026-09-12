use scoop_wire::{HashError, WireEncode, domain_separated_cbor_hash};

use super::PendingIdentityResolver;
use crate::ids::PersistentIdConstruction;
use crate::{
    CallableApplicationKey, CallbackApplicationKey, CallbackRegistrationKey, CborIdentityKey,
    DecodedCallableApplicationKey, DecodedCallbackApplicationKey, DecodedCallbackRegistrationKey,
    DecodedDispatchSlotKey, DecodedDispatchTableKey, DecodedEnumVariantFieldKey,
    DecodedEnumVariantIdentityKey, DecodedExactTypeKey, DecodedExportBindingKey,
    DecodedFieldIdentityKey, DecodedGeneratedBridgeAtomKey, DecodedGeneratedBridgeUnitKey,
    DecodedGeneratedCallableKey, DecodedGeneratedNominalKey, DecodedImmortalObjectKey,
    DecodedInitializationUnitKey, DecodedLayoutKey, DecodedLocalBindingKey, DecodedLocalValueKey,
    DecodedNativeLinkRequirementKey, DecodedObjectDefinitionAtomKey,
    DecodedObjectDefinitionPlanKey, DecodedOdrMemberKey, DecodedPropertyAccessorKey,
    DecodedSafepointSiteKey, DecodedScanKey, DecodedSourceContextKey, DecodedSourceDeclarationKey,
    DecodedSpecializationKey, DecodedStaticStorageKey, DispatchSlotKey, DispatchTableKey,
    EnumVariantFieldKey, EnumVariantIdentityKey, ExactTypeKey, ExportBindingKey, FieldIdentityKey,
    GeneratedBridgeAtomId, GeneratedBridgeAtomKey, GeneratedBridgeUnitId, GeneratedBridgeUnitKey,
    GeneratedCallableKey, GeneratedNominalKey, ImmortalObjectKey, InitializationUnitKey, LayoutKey,
    LocalBindingKey, LocalValueKey, NativeLinkRequirementId, NativeLinkRequirementKey,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, OdrGroupId, OdrMemberId, OdrMemberKey,
    PersistentCallableApplicationId, PersistentCallbackApplicationId,
    PersistentCallbackRegistrationId, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentExtensionPropertyId,
    PersistentFieldId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentLayoutId, PersistentLocalBindingId,
    PersistentLocalValueId, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentSafepointSiteId, PersistentScanId, PersistentSourceContextId,
    PersistentStaticStorageId, PersistentTypeAliasId, PersistentTypeId, PropertyAccessorKey,
    SafepointSiteKey, ScanKey, SourceContextKey, SourceDeclarationKey, SpecializationKey,
    StaticStorageKey,
};

mod private {
    pub trait Sealed<I> {}
}

/// Exact relationship between an untrusted decoded key and its canonical key.
///
/// The trait is sealed: artifact readers can use the implementations supplied
/// by this crate but cannot declare an alternative hash preimage or resolver.
pub trait DecodedIdentityKey<I: PersistentId>: private::Sealed<I> + Clone + WireEncode {
    type Canonical: CborIdentityKey<I> + Clone + Eq + 'static;

    fn candidate_identity(&self) -> Result<I, HashError>;

    #[doc(hidden)]
    fn resolve_identity_key(
        self,
        resolver: &mut PendingIdentityResolver<'_>,
    ) -> Result<Self::Canonical, String>;
}

fn derive_candidate<I: PersistentIdConstruction>(
    domain: &'static str,
    key: &impl WireEncode,
) -> Result<I, HashError> {
    domain_separated_cbor_hash(domain, key).map(I::from_digest)
}

macro_rules! decoded_key {
    ($decoded:ty => $canonical:ty, $id:ty, $domain:literal) => {
        impl private::Sealed<$id> for $decoded {}

        impl DecodedIdentityKey<$id> for $decoded {
            type Canonical = $canonical;

            fn candidate_identity(&self) -> Result<$id, HashError> {
                derive_candidate($domain, self)
            }

            fn resolve_identity_key(
                self,
                resolver: &mut PendingIdentityResolver<'_>,
            ) -> Result<Self::Canonical, String> {
                self.resolve(resolver).map_err(|error| error.to_string())
            }
        }
    };
}

macro_rules! source_declaration_key {
    ($id:ty, $domain:literal) => {
        impl private::Sealed<$id> for DecodedSourceDeclarationKey {}

        impl DecodedIdentityKey<$id> for DecodedSourceDeclarationKey {
            type Canonical = SourceDeclarationKey;

            fn candidate_identity(&self) -> Result<$id, HashError> {
                derive_candidate($domain, self)
            }

            fn resolve_identity_key(
                self,
                resolver: &mut PendingIdentityResolver<'_>,
            ) -> Result<Self::Canonical, String> {
                self.resolve(resolver).map_err(|error| error.to_string())
            }
        }
    };
}

struct SourceTypeCandidate<'key>(&'key DecodedSourceDeclarationKey);

impl WireEncode for SourceTypeCandidate<'_> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(1)?;
        encoder.field(1)?;
        self.0.encode(encoder)
    }
}

impl private::Sealed<PersistentTypeId> for DecodedSourceDeclarationKey {}

impl DecodedIdentityKey<PersistentTypeId> for DecodedSourceDeclarationKey {
    type Canonical = SourceDeclarationKey;

    fn candidate_identity(&self) -> Result<PersistentTypeId, HashError> {
        derive_candidate("scoop-type-id-v1", &SourceTypeCandidate(self))
    }

    fn resolve_identity_key(
        self,
        resolver: &mut PendingIdentityResolver<'_>,
    ) -> Result<Self::Canonical, String> {
        self.resolve(resolver).map_err(|error| error.to_string())
    }
}

struct GeneratedTypeCandidate<'key>(&'key DecodedGeneratedNominalKey);

impl WireEncode for GeneratedTypeCandidate<'_> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(2)?;
        encoder.field(1)?;
        self.0.encode(encoder)
    }
}

impl private::Sealed<PersistentTypeId> for DecodedGeneratedNominalKey {}

impl DecodedIdentityKey<PersistentTypeId> for DecodedGeneratedNominalKey {
    type Canonical = GeneratedNominalKey;

    fn candidate_identity(&self) -> Result<PersistentTypeId, HashError> {
        derive_candidate("scoop-type-id-v1", &GeneratedTypeCandidate(self))
    }

    fn resolve_identity_key(
        self,
        resolver: &mut PendingIdentityResolver<'_>,
    ) -> Result<Self::Canonical, String> {
        self.resolve(resolver).map_err(|error| error.to_string())
    }
}

struct ObjectValueCandidate(PersistentTypeId);

impl WireEncode for ObjectValueCandidate {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.0.encode(encoder)
    }
}

impl private::Sealed<PersistentObjectValueId> for DecodedSourceDeclarationKey {}

impl DecodedIdentityKey<PersistentObjectValueId> for DecodedSourceDeclarationKey {
    type Canonical = SourceDeclarationKey;

    fn candidate_identity(&self) -> Result<PersistentObjectValueId, HashError> {
        let source_type = derive_candidate("scoop-type-id-v1", &SourceTypeCandidate(self))?;
        derive_candidate(
            "scoop-object-value-id-v1",
            &ObjectValueCandidate(source_type),
        )
    }

    fn resolve_identity_key(
        self,
        resolver: &mut PendingIdentityResolver<'_>,
    ) -> Result<Self::Canonical, String> {
        self.resolve(resolver).map_err(|error| error.to_string())
    }
}

source_declaration_key!(PersistentGenericTypeId, "scoop-generic-type-id-v1");
source_declaration_key!(PersistentFunctionId, "scoop-function-id-v1");
source_declaration_key!(PersistentGenericFunctionId, "scoop-generic-function-id-v1");
source_declaration_key!(PersistentConstructorId, "scoop-constructor-id-v1");
source_declaration_key!(PersistentPropertyId, "scoop-property-id-v1");
source_declaration_key!(
    PersistentExtensionPropertyId,
    "scoop-extension-property-id-v1"
);
source_declaration_key!(PersistentTypeAliasId, "scoop-type-alias-id-v1");

decoded_key!(DecodedPropertyAccessorKey => PropertyAccessorKey, PersistentPropertyAccessorId, "scoop-property-accessor-id-v1");
decoded_key!(DecodedFieldIdentityKey => FieldIdentityKey, PersistentFieldId, "scoop-field-id-v1");
decoded_key!(DecodedEnumVariantIdentityKey => EnumVariantIdentityKey, PersistentEnumVariantId, "scoop-enum-variant-id-v1");
decoded_key!(DecodedEnumVariantFieldKey => EnumVariantFieldKey, PersistentEnumVariantFieldId, "scoop-enum-variant-field-id-v1");
decoded_key!(DecodedExactTypeKey => ExactTypeKey, PersistentExactTypeId, "scoop-exact-type-v1");
decoded_key!(DecodedExportBindingKey => ExportBindingKey, PersistentExportBindingId, "scoop-export-binding-id-v1");
decoded_key!(DecodedCallableApplicationKey => CallableApplicationKey, PersistentCallableApplicationId, "scoop-callable-application-id-v1");
decoded_key!(DecodedGeneratedCallableKey => GeneratedCallableKey, PersistentGeneratedCallableId, "scoop-generated-callable-id-v1");
decoded_key!(DecodedDispatchSlotKey => DispatchSlotKey, PersistentDispatchSlotId, "scoop-dispatch-slot-id-v1");
decoded_key!(DecodedInitializationUnitKey => InitializationUnitKey, PersistentInitializationUnitId, "scoop-initialization-unit-id-v1");
decoded_key!(DecodedSourceContextKey => SourceContextKey, PersistentSourceContextId, "scoop-source-context-id-v1");
decoded_key!(DecodedLocalBindingKey => LocalBindingKey, PersistentLocalBindingId, "scoop-local-binding-id-v1");
decoded_key!(DecodedLocalValueKey => LocalValueKey, PersistentLocalValueId, "scoop-local-value-id-v1");
decoded_key!(DecodedCallbackRegistrationKey => CallbackRegistrationKey, PersistentCallbackRegistrationId, "scoop-callback-registration-id-v1");
decoded_key!(DecodedCallbackApplicationKey => CallbackApplicationKey, PersistentCallbackApplicationId, "scoop-callback-application-id-v1");
decoded_key!(DecodedSpecializationKey => SpecializationKey, OdrGroupId, "scoop-odr-v1");
decoded_key!(DecodedOdrMemberKey => OdrMemberKey, OdrMemberId, "scoop-odr-member-v1");
decoded_key!(DecodedLayoutKey => LayoutKey, PersistentLayoutId, "scoop-layout-id-v1");
decoded_key!(DecodedScanKey => ScanKey, PersistentScanId, "scoop-scan-id-v1");
decoded_key!(DecodedDispatchTableKey => DispatchTableKey, PersistentDispatchTableId, "scoop-dispatch-table-id-v1");
decoded_key!(DecodedStaticStorageKey => StaticStorageKey, PersistentStaticStorageId, "scoop-static-storage-id-v1");
decoded_key!(DecodedImmortalObjectKey => ImmortalObjectKey, PersistentImmortalObjectId, "scoop-immortal-object-id-v1");
decoded_key!(DecodedSafepointSiteKey => SafepointSiteKey, PersistentSafepointSiteId, "scoop-safepoint-site-v1");
decoded_key!(DecodedGeneratedBridgeUnitKey => GeneratedBridgeUnitKey, GeneratedBridgeUnitId, "scoop-generated-bridge-unit-v1");
decoded_key!(DecodedGeneratedBridgeAtomKey => GeneratedBridgeAtomKey, GeneratedBridgeAtomId, "scoop-generated-bridge-atom-v1");
decoded_key!(DecodedObjectDefinitionPlanKey => ObjectDefinitionPlanKey, ObjectDefinitionPlanId, "scoop-object-definition-plan-v1");
decoded_key!(DecodedObjectDefinitionAtomKey => ObjectDefinitionAtomKey, ObjectDefinitionAtomId, "scoop-object-definition-atom-v1");

impl private::Sealed<NativeLinkRequirementId> for DecodedNativeLinkRequirementKey {}

impl DecodedIdentityKey<NativeLinkRequirementId> for DecodedNativeLinkRequirementKey {
    type Canonical = NativeLinkRequirementKey;

    fn candidate_identity(&self) -> Result<NativeLinkRequirementId, HashError> {
        derive_candidate("scoop-native-link-requirement-v1", self)
    }

    fn resolve_identity_key(
        self,
        _resolver: &mut PendingIdentityResolver<'_>,
    ) -> Result<Self::Canonical, String> {
        self.validate().map_err(|error| error.to_string())
    }
}
