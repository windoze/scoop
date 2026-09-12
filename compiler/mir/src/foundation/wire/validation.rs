use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    CallableOdrMemberId, CallableOwner, CallbackRegistrationKey, DecodedPersistentId,
    IdentityLayer, IdentityReferenceError, IdentityValidationError, OdrMemberDiscriminator,
    OdrMemberId, OdrMemberIdentityError, OdrMemberKey, PersistentCallbackRegistrationId,
    PersistentId, PersistentIdResolver, PersistentKeyResolver, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WireEncode, WirePath, encode};

use super::*;
use crate::{
    CallableSignatureResolutionError, CallableSignatureResolver, CallableSignatureSubject,
    CallbackApplicationResolutionError,
};

/// MIR foundation reconstructed from validated identities and checked
/// non-identity relations.
#[derive(Debug)]
pub struct ValidatedMirFoundation {
    canonical: CanonicalMirFoundation,
}

impl ValidatedMirFoundation {
    pub fn counts(&self) -> MirFoundationCounts {
        self.canonical.counts()
    }

    pub(crate) fn into_canonical(self) -> CanonicalMirFoundation {
        self.canonical
    }

    #[doc(hidden)]
    pub fn callback_application_records(&self) -> &[CallbackApplicationRecord] {
        &self.canonical.callback_application_records
    }
}

impl WireEncode for ValidatedMirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

impl DecodedMirFoundation {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<ValidatedMirFoundation, MirFoundationValidationError> {
        validate_foundation(self, identities, meter)
    }
}

fn validate_foundation(
    foundation: DecodedMirFoundation,
    identities: &mut ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<ValidatedMirFoundation, MirFoundationValidationError> {
    let original = encode(&foundation).map_err(MirFoundationValidationError::WireEncode)?;
    let DecodedMirFoundationWire {
        exact_types: _,
        generated_callables: _,
        generated_types: _,
        fields: _,
        enum_variants: _,
        enum_variant_fields: _,
        callable_signatures,
        local_values: _,
        callback_applications: _,
        callback_application_records,
        odr_groups: _,
        odr_members: _,
    } = foundation.decoded;

    macro_rules! records {
        ($field:literal, $id:ty, $key:ty) => {
            identities
                .records::<$id, $key>(IdentityLayer::Mir, meter, &WirePath::root().field($field))
                .map_err(MirFoundationValidationError::Identity)?
        };
    }
    let exact_types: Vec<ExactTypeRecord> = records!(1, PersistentExactTypeId, ExactTypeKey);
    let generated_callables: Vec<GeneratedCallableRecord> =
        records!(2, PersistentGeneratedCallableId, GeneratedCallableKey);
    let generated_types: Vec<GeneratedTypeRecord> =
        records!(3, PersistentTypeId, GeneratedNominalKey);
    let fields: Vec<FieldRecord> = records!(4, PersistentFieldId, FieldIdentityKey);
    let enum_variants: Vec<EnumVariantRecord> =
        records!(5, PersistentEnumVariantId, EnumVariantIdentityKey);
    let enum_variant_fields: Vec<EnumVariantFieldRecord> =
        records!(6, PersistentEnumVariantFieldId, EnumVariantFieldKey);
    let local_values: Vec<LocalValueRecord> = records!(8, PersistentLocalValueId, LocalValueKey);
    let callback_applications: Vec<CallbackApplicationIdentityRecord> =
        records!(9, PersistentCallbackApplicationId, CallbackApplicationKey);
    let odr_groups: Vec<OdrGroupRecord> = records!(11, OdrGroupId, SpecializationKey);
    let odr_members: Vec<OdrMemberRecord> = records!(12, OdrMemberId, OdrMemberKey);

    let mut resolver = FoundationResolver { identities };
    let mut signatures = Vec::new();
    signatures
        .try_reserve_exact(callable_signatures.len())
        .map_err(|_| MirFoundationValidationError::Allocation)?;
    for (index, signature) in callable_signatures.into_iter().enumerate() {
        signatures.push(
            signature.resolve(&mut resolver).map_err(|error| {
                MirFoundationValidationError::CallableSignature { index, error }
            })?,
        );
    }
    let mut applications = Vec::new();
    applications
        .try_reserve_exact(callback_application_records.len())
        .map_err(|_| MirFoundationValidationError::Allocation)?;
    for (index, application) in callback_application_records.into_iter().enumerate() {
        applications.push(
            application.resolve(&mut resolver).map_err(|error| {
                MirFoundationValidationError::CallbackApplication { index, error }
            })?,
        );
    }
    validate_callback_applications(
        resolver.identities,
        &callback_applications,
        &applications,
        &signatures,
        meter,
    )?;

    let mut canonical = CanonicalMirFoundation::empty();
    macro_rules! set {
        ($method:ident, $records:expr) => {
            canonical
                .$method($records)
                .map_err(MirFoundationValidationError::Build)?
        };
    }
    set!(set_exact_types, exact_types);
    set!(set_generated_callables, generated_callables);
    set!(set_generated_types, generated_types);
    set!(set_fields, fields);
    set!(set_enum_variants, enum_variants);
    set!(set_enum_variant_fields, enum_variant_fields);
    set!(set_callable_signatures, signatures);
    set!(set_local_values, local_values);
    set!(set_callback_applications, callback_applications);
    set!(set_callback_application_records, applications);
    set!(set_odr_groups, odr_groups);
    set!(set_odr_members, odr_members);

    let rebuilt = encode(&canonical).map_err(MirFoundationValidationError::WireEncode)?;
    if rebuilt != original {
        return Err(MirFoundationValidationError::NonCanonicalFoundation);
    }
    Ok(ValidatedMirFoundation { canonical })
}

struct FoundationResolver<'a> {
    identities: &'a mut ValidatedIdentityGraph,
}

impl<I> PersistentIdResolver<I> for FoundationResolver<'_>
where
    I: PersistentId,
    ValidatedIdentityGraph: PersistentIdResolver<I, Error = IdentityReferenceError>,
{
    type Error = MirFoundationReferenceError;

    fn resolve(&mut self, id: DecodedPersistentId<I>) -> Result<I, Self::Error> {
        <ValidatedIdentityGraph as PersistentIdResolver<I>>::resolve(self.identities, id)
            .map_err(MirFoundationReferenceError::Identity)
    }
}

impl CallableSignatureResolver<MirFoundationReferenceError> for FoundationResolver<'_> {
    fn resolve_callable_odr_member(
        &mut self,
        member: DecodedPersistentId<OdrMemberId>,
    ) -> Result<CallableOdrMemberId, MirFoundationReferenceError> {
        let key = <ValidatedIdentityGraph as PersistentKeyResolver<
            OdrMemberId,
            OdrMemberKey,
        >>::resolve_key(self.identities, member)
        .map_err(MirFoundationReferenceError::Identity)?;
        CallableOdrMemberId::from_key(&key).map_err(MirFoundationReferenceError::CallableOdrMember)
    }
}

fn validate_callback_applications(
    identities: &ValidatedIdentityGraph,
    identity_records: &[CallbackApplicationIdentityRecord],
    records: &[CallbackApplicationRecord],
    signatures: &[CallableSignatureRecord],
    meter: &mut BudgetMeter,
) -> Result<(), MirFoundationValidationError> {
    let identity_keys = identity_records
        .iter()
        .map(|record| (record.id(), record.key()))
        .collect::<BTreeMap<_, _>>();
    let registrations = identities
        .records::<PersistentCallbackRegistrationId, CallbackRegistrationKey>(
            IdentityLayer::Hir,
            meter,
            &WirePath::root().field(25),
        )
        .map_err(MirFoundationValidationError::Identity)?
        .into_iter()
        .map(|record| (record.id(), record))
        .collect::<BTreeMap<_, _>>();
    let mut generated = identities
        .records::<PersistentGeneratedCallableId, GeneratedCallableKey>(
            IdentityLayer::Hir,
            meter,
            &WirePath::root().field(18),
        )
        .map_err(MirFoundationValidationError::Identity)?;
    generated.extend(
        identities
            .records::<PersistentGeneratedCallableId, GeneratedCallableKey>(
                IdentityLayer::Mir,
                meter,
                &WirePath::root().field(2),
            )
            .map_err(MirFoundationValidationError::Identity)?,
    );
    let generated = generated
        .into_iter()
        .map(|record| (record.id(), record))
        .collect::<BTreeMap<_, _>>();
    let mut members = identities
        .records::<OdrMemberId, OdrMemberKey>(
            IdentityLayer::Hir,
            meter,
            &WirePath::root().field(28),
        )
        .map_err(MirFoundationValidationError::Identity)?;
    members.extend(
        identities
            .records::<OdrMemberId, OdrMemberKey>(
                IdentityLayer::Mir,
                meter,
                &WirePath::root().field(12),
            )
            .map_err(MirFoundationValidationError::Identity)?,
    );
    let members = members
        .into_iter()
        .map(|record| (record.id(), record))
        .collect::<BTreeMap<_, _>>();

    let mut seen = BTreeMap::new();
    for record in records {
        let application = record.application();
        if seen.insert(application, ()).is_some() {
            return Err(CallbackApplicationRelationError::DuplicateRecord { application }.into());
        }
        let key = identity_keys
            .get(&application)
            .ok_or(CallbackApplicationRelationError::UnexpectedRecord { application })?;
        let registration = registrations.get(&key.registration()).ok_or(
            CallbackApplicationRelationError::MissingRegistration {
                application,
                registration: key.registration(),
            },
        )?;
        if record.mode() != registration.key().mode() {
            return Err(CallbackApplicationRelationError::ModeMismatch { application }.into());
        }
        let signature = signatures
            .iter()
            .find(|signature| signature.subject() == record.managed_adapter())
            .ok_or(CallbackApplicationRelationError::MissingManagedSignature {
                application,
                subject: record.managed_adapter(),
            })?;
        if signature.signature() != record.managed_signature() {
            return Err(
                CallbackApplicationRelationError::ManagedSignatureMismatch { application }.into(),
            );
        }
        let adapter = managed_adapter_identity(record.managed_adapter(), &members).ok_or(
            CallbackApplicationRelationError::InvalidManagedAdapter {
                application,
                subject: record.managed_adapter(),
            },
        )?;
        if !matches!(
            generated.get(&adapter).map(|record| record.key()),
            Some(GeneratedCallableKey::ForeignCallbackManagedAdapter { application: owner })
                if *owner == application
        ) {
            return Err(CallbackApplicationRelationError::InvalidManagedAdapter {
                application,
                subject: record.managed_adapter(),
            }
            .into());
        }
    }
    if let Some(application) = identity_keys
        .keys()
        .find(|application| !seen.contains_key(application))
    {
        return Err(CallbackApplicationRelationError::MissingRecord {
            application: *application,
        }
        .into());
    }
    Ok(())
}

fn managed_adapter_identity(
    subject: CallableSignatureSubject,
    members: &BTreeMap<OdrMemberId, CborIdentityRecord<OdrMemberId, OdrMemberKey>>,
) -> Option<PersistentGeneratedCallableId> {
    match subject {
        CallableSignatureSubject::Strong(CallableOwner::Generated(id)) => Some(id),
        CallableSignatureSubject::Odr(member) => {
            match members.get(&member.member())?.key().discriminator() {
                OdrMemberDiscriminator::GeneratedCallable(id) => Some(*id),
                _ => None,
            }
        }
        CallableSignatureSubject::Strong(
            CallableOwner::Function(_)
            | CallableOwner::GenericTemplate(_)
            | CallableOwner::Application(_)
            | CallableOwner::Constructor(_)
            | CallableOwner::Accessor(_),
        ) => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirFoundationReferenceError {
    Identity(IdentityReferenceError),
    CallableOdrMember(OdrMemberIdentityError),
}

impl fmt::Display for MirFoundationReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::CallableOdrMember(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for MirFoundationReferenceError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackApplicationRelationError {
    DuplicateRecord {
        application: PersistentCallbackApplicationId,
    },
    UnexpectedRecord {
        application: PersistentCallbackApplicationId,
    },
    MissingRecord {
        application: PersistentCallbackApplicationId,
    },
    MissingRegistration {
        application: PersistentCallbackApplicationId,
        registration: PersistentCallbackRegistrationId,
    },
    ModeMismatch {
        application: PersistentCallbackApplicationId,
    },
    MissingManagedSignature {
        application: PersistentCallbackApplicationId,
        subject: CallableSignatureSubject,
    },
    ManagedSignatureMismatch {
        application: PersistentCallbackApplicationId,
    },
    InvalidManagedAdapter {
        application: PersistentCallbackApplicationId,
        subject: CallableSignatureSubject,
    },
}

impl fmt::Display for CallbackApplicationRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRecord { application } => {
                write!(
                    formatter,
                    "callback application {application} has multiple MIR records"
                )
            }
            Self::UnexpectedRecord { application } => write!(
                formatter,
                "MIR callback record refers to undeclared application {application}"
            ),
            Self::MissingRecord { application } => {
                write!(
                    formatter,
                    "callback application {application} has no MIR record"
                )
            }
            Self::MissingRegistration {
                application,
                registration,
            } => write!(
                formatter,
                "callback application {application} refers to absent HIR registration {registration}"
            ),
            Self::ModeMismatch { application } => write!(
                formatter,
                "callback application {application} changes its HIR registration mode"
            ),
            Self::MissingManagedSignature {
                application,
                subject,
            } => write!(
                formatter,
                "callback application {application} adapter {subject:?} has no MIR signature"
            ),
            Self::ManagedSignatureMismatch { application } => write!(
                formatter,
                "callback application {application} and its adapter record different signatures"
            ),
            Self::InvalidManagedAdapter {
                application,
                subject,
            } => write!(
                formatter,
                "callback application {application} names non-canonical managed adapter {subject:?}"
            ),
        }
    }
}

impl std::error::Error for CallbackApplicationRelationError {}

#[derive(Debug)]
pub enum MirFoundationValidationError {
    WireEncode(scoop_wire::cbor::EncodeError),
    Allocation,
    Identity(IdentityValidationError),
    CallableSignature {
        index: usize,
        error: CallableSignatureResolutionError<MirFoundationReferenceError>,
    },
    CallbackApplication {
        index: usize,
        error: CallbackApplicationResolutionError<MirFoundationReferenceError>,
    },
    CallbackRelation(CallbackApplicationRelationError),
    Build(MirFoundationBuildError),
    NonCanonicalFoundation,
}

impl From<CallbackApplicationRelationError> for MirFoundationValidationError {
    fn from(error: CallbackApplicationRelationError) -> Self {
        Self::CallbackRelation(error)
    }
}

impl fmt::Display for MirFoundationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WireEncode(error) => error.fmt(formatter),
            Self::Allocation => formatter.write_str("failed to allocate MIR foundation validation"),
            Self::Identity(error) => error.fmt(formatter),
            Self::CallableSignature { index, error } => {
                write!(
                    formatter,
                    "MIR callable signature {index} is invalid: {error}"
                )
            }
            Self::CallbackApplication { index, error } => {
                write!(
                    formatter,
                    "MIR callback application {index} is invalid: {error}"
                )
            }
            Self::CallbackRelation(error) => error.fmt(formatter),
            Self::Build(error) => error.fmt(formatter),
            Self::NonCanonicalFoundation => {
                formatter.write_str("MIR foundation tables are not in canonical structural order")
            }
        }
    }
}

impl std::error::Error for MirFoundationValidationError {}

#[cfg(test)]
mod tests;
