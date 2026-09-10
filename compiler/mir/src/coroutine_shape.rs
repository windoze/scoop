//! Persistent identity bundles for exact coroutine step and slot shapes.

use std::fmt;

use scoop_identity::{
    CborIdentityRecord, EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityError,
    EnumVariantIdentityKey, ExactTypeKey, GeneratedEnumVariantRole, GeneratedNominalIdentityError,
    GeneratedNominalKey, OdrGroupId, OdrMemberDiscriminator, OdrMemberRole,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId, PersistentTypeId,
    SpecializationKey,
};
use scoop_wire::HashError;

use crate::{ExactOwnerRoot, ExactOwnerRootError};

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type VariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type VariantFieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineStepIdentity {
    result: ExactTypeRecord,
    generated_type: GeneratedTypeRecord,
    completed: VariantRecord,
    completed_payload: VariantFieldRecord,
    suspended: VariantRecord,
    root: ExactOwnerRoot,
}

impl CoroutineStepIdentity {
    pub fn new(
        result: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
    ) -> Result<Self, CoroutineShapeIdentityError> {
        let generated_type = generated_type(GeneratedNominalKey::CoroutineStep {
            result: result.id(),
        })?;
        let completed = variant(
            generated_type.key(),
            GeneratedEnumVariantRole::CoroutineStepCompleted,
        )?;
        let completed_payload = positional_payload(completed.id())?;
        let suspended = variant(
            generated_type.key(),
            GeneratedEnumVariantRole::CoroutineStepSuspended,
        )?;
        let root = root(result, nominal_group, generated_type.id())?;
        Ok(Self {
            result: result.clone(),
            generated_type,
            completed,
            completed_payload,
            suspended,
            root,
        })
    }

    pub const fn result_record(&self) -> &ExactTypeRecord {
        &self.result
    }

    pub const fn generated_type_record(&self) -> &GeneratedTypeRecord {
        &self.generated_type
    }

    pub const fn completed_variant_record(&self) -> &VariantRecord {
        &self.completed
    }

    pub const fn completed_payload_record(&self) -> &VariantFieldRecord {
        &self.completed_payload
    }

    pub const fn suspended_variant_record(&self) -> &VariantRecord {
        &self.suspended
    }

    pub const fn root(&self) -> &ExactOwnerRoot {
        &self.root
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineSlotIdentity {
    value: ExactTypeRecord,
    generated_type: GeneratedTypeRecord,
    empty: VariantRecord,
    value_variant: VariantRecord,
    value_payload: VariantFieldRecord,
    root: ExactOwnerRoot,
}

impl CoroutineSlotIdentity {
    pub fn new(
        value: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
    ) -> Result<Self, CoroutineShapeIdentityError> {
        let generated_type =
            generated_type(GeneratedNominalKey::CoroutineSlot { value: value.id() })?;
        let empty = variant(
            generated_type.key(),
            GeneratedEnumVariantRole::CoroutineSlotEmpty,
        )?;
        let value_variant = variant(
            generated_type.key(),
            GeneratedEnumVariantRole::CoroutineSlotValue,
        )?;
        let value_payload = positional_payload(value_variant.id())?;
        let root = root(value, nominal_group, generated_type.id())?;
        Ok(Self {
            value: value.clone(),
            generated_type,
            empty,
            value_variant,
            value_payload,
            root,
        })
    }

    pub const fn value_record(&self) -> &ExactTypeRecord {
        &self.value
    }

    pub const fn generated_type_record(&self) -> &GeneratedTypeRecord {
        &self.generated_type
    }

    pub const fn empty_variant_record(&self) -> &VariantRecord {
        &self.empty
    }

    pub const fn value_variant_record(&self) -> &VariantRecord {
        &self.value_variant
    }

    pub const fn value_payload_record(&self) -> &VariantFieldRecord {
        &self.value_payload
    }

    pub const fn root(&self) -> &ExactOwnerRoot {
        &self.root
    }
}

fn generated_type(
    key: GeneratedNominalKey,
) -> Result<GeneratedTypeRecord, CoroutineShapeIdentityError> {
    CborIdentityRecord::from_key(key).map_err(CoroutineShapeIdentityError::GeneratedType)
}

fn variant(
    owner: &GeneratedNominalKey,
    role: GeneratedEnumVariantRole,
) -> Result<VariantRecord, CoroutineShapeIdentityError> {
    let key = EnumVariantIdentityKey::generated(owner, role)
        .map_err(CoroutineShapeIdentityError::Variant)?;
    CborIdentityRecord::from_key(key).map_err(CoroutineShapeIdentityError::Variant)
}

fn positional_payload(
    variant: PersistentEnumVariantId,
) -> Result<VariantFieldRecord, CoroutineShapeIdentityError> {
    CborIdentityRecord::from_key(EnumVariantFieldKey::new(
        variant,
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))
    .map_err(CoroutineShapeIdentityError::VariantField)
}

fn root(
    exact: &ExactTypeRecord,
    nominal_group: Option<&OdrGroupRecord>,
    generated_type: PersistentTypeId,
) -> Result<ExactOwnerRoot, CoroutineShapeIdentityError> {
    ExactOwnerRoot::for_member(
        exact,
        nominal_group,
        OdrMemberRole::GeneratedNominal,
        OdrMemberDiscriminator::GeneratedNominal(generated_type),
    )
    .map_err(CoroutineShapeIdentityError::Root)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoroutineShapeIdentityError {
    GeneratedType(GeneratedNominalIdentityError),
    Variant(EnumVariantIdentityError),
    VariantField(HashError),
    Root(ExactOwnerRootError),
}

impl fmt::Display for CoroutineShapeIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedType(error) => error.fmt(formatter),
            Self::Variant(error) => error.fmt(formatter),
            Self::VariantField(error) => error.fmt(formatter),
            Self::Root(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoroutineShapeIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, Effect, NonEmptyVec, PackagePath, PersistentGenericTypeId,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;

    fn unit() -> ExactTypeRecord {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }

    fn generic_origin() -> PersistentGenericTypeId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        PersistentGenericTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
            site,
            CanonicalIdentifier::new("Result").unwrap(),
            SourceNominalKind::Struct,
            1,
        ))
        .unwrap()
    }

    #[test]
    fn step_records_both_variants_and_its_positional_payload() {
        let identity = CoroutineStepIdentity::new(&unit(), None).unwrap();
        assert!(matches!(
            identity.generated_type_record().key(),
            GeneratedNominalKey::CoroutineStep { result } if *result == unit().id()
        ));
        assert_eq!(
            identity.completed_payload_record().key().variant(),
            identity.completed_variant_record().id()
        );
        assert_eq!(
            identity.completed_payload_record().key().selector(),
            &EnumVariantFieldSelector::Positional {
                declaration_index: 0
            }
        );
        assert_ne!(
            identity.completed_variant_record().id(),
            identity.suspended_variant_record().id()
        );
    }

    #[test]
    fn slot_records_empty_value_and_its_positional_payload() {
        let identity = CoroutineSlotIdentity::new(&unit(), None).unwrap();
        assert!(matches!(
            identity.generated_type_record().key(),
            GeneratedNominalKey::CoroutineSlot { value } if *value == unit().id()
        ));
        assert_eq!(
            identity.value_payload_record().key().variant(),
            identity.value_variant_record().id()
        );
        assert_ne!(
            identity.empty_variant_record().id(),
            identity.value_variant_record().id()
        );
    }

    #[test]
    fn nominal_application_shapes_reuse_the_hir_group() {
        let origin = generic_origin();
        let arguments = NonEmptyVec::from_first(unit().id(), []);
        let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin,
            arguments: arguments.clone(),
        })
        .unwrap();
        let group =
            CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
        for root in [
            CoroutineStepIdentity::new(&exact, Some(&group))
                .unwrap()
                .root(),
            CoroutineSlotIdentity::new(&exact, Some(&group))
                .unwrap()
                .root(),
        ] {
            let ExactOwnerRoot::NominalApplication(root) = root else {
                panic!("nominal applications reuse their HIR group")
            };
            assert_eq!(root.group(), group.id());
        }
    }

    #[test]
    fn structural_function_shapes_create_exact_groups() {
        let exact = CborIdentityRecord::from_key(ExactTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: Vec::new(),
            result: unit().id(),
        })
        .unwrap();
        let step = CoroutineStepIdentity::new(&exact, None).unwrap();
        let slot = CoroutineSlotIdentity::new(&exact, None).unwrap();
        for root in [step.root(), slot.root()] {
            let ExactOwnerRoot::Structural(root) = root else {
                panic!("function shapes create structural groups")
            };
            assert_eq!(
                root.group_record().key(),
                &SpecializationKey::StructuralType {
                    exact_type: exact.id()
                }
            );
        }
    }
}
