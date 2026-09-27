use std::collections::{HashMap, HashSet};
use std::fmt;

use scoop_identity::{
    CallableOdrMemberId, CallableOwner, CallbackRegistrationKey, DecodedPersistentId,
    IdentityLayer, IdentityReferenceError, IdentityValidationError, OdrMemberDiscriminator,
    OdrMemberId, OdrMemberIdentityError, OdrMemberKey, PersistentCallbackRegistrationId,
    PersistentId, PersistentIdResolver, PersistentKeyResolver, ValidatedIdentityGraph,
};
use scoop_wire::{WireEncode, WirePath, encode_canonical_temporary};

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

    pub(crate) const fn canonical(&self) -> &CanonicalMirFoundation {
        &self.canonical
    }

    pub fn into_canonical(self) -> CanonicalMirFoundation {
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
    ) -> Result<ValidatedMirFoundation, MirFoundationValidationError> {
        validate_foundation(self, identities)
    }
}

fn validate_foundation(
    foundation: DecodedMirFoundation,
    identities: &mut ValidatedIdentityGraph,
) -> Result<ValidatedMirFoundation, MirFoundationValidationError> {
    let original = encode_canonical_temporary(&foundation, &WirePath::root())
        .map_err(MirFoundationValidationError::Resource)?;
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
                .records::<$id, $key>(IdentityLayer::Mir, &WirePath::root().field($field))
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
    scoop_wire::allocation::try_reserve(
        &mut signatures,
        callable_signatures.len(),
        &WirePath::root().field(7),
    )
    .map_err(MirFoundationValidationError::Resource)?;
    for (index, signature) in callable_signatures.into_iter().enumerate() {
        signatures.push(
            signature.resolve(&mut resolver).map_err(|error| {
                MirFoundationValidationError::CallableSignature { index, error }
            })?,
        );
    }
    let mut applications = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut applications,
        callback_application_records.len(),
        &WirePath::root().field(10),
    )
    .map_err(MirFoundationValidationError::Resource)?;
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

    let rebuilt = encode_canonical_temporary(&canonical, &WirePath::root())
        .map_err(MirFoundationValidationError::Resource)?;
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
) -> Result<(), MirFoundationValidationError> {
    let identity_path = WirePath::root().field(9);
    let mut identity_keys = HashMap::new();
    scoop_wire::allocation::try_reserve_map(
        &mut identity_keys,
        identity_records.len(),
        &identity_path,
    )
    .map_err(MirFoundationValidationError::Resource)?;
    identity_keys.extend(
        identity_records
            .iter()
            .map(|record| (record.id(), record.key())),
    );

    let registration_records = identities
        .records::<PersistentCallbackRegistrationId, CallbackRegistrationKey>(
            IdentityLayer::Hir,
            &WirePath::root().field(25),
        )
        .map_err(MirFoundationValidationError::Identity)?;
    let registration_path = WirePath::root().field(10);
    let mut registrations = HashMap::new();
    scoop_wire::allocation::try_reserve_map(
        &mut registrations,
        registration_records.len(),
        &registration_path,
    )
    .map_err(MirFoundationValidationError::Resource)?;
    registrations.extend(
        registration_records
            .iter()
            .map(|record| (record.id(), record.key())),
    );

    let hir_generated = identities
        .records::<PersistentGeneratedCallableId, GeneratedCallableKey>(
            IdentityLayer::Hir,
            &WirePath::root().field(18),
        )
        .map_err(MirFoundationValidationError::Identity)?;
    let mir_generated = identities
        .records::<PersistentGeneratedCallableId, GeneratedCallableKey>(
            IdentityLayer::Mir,
            &WirePath::root().field(2),
        )
        .map_err(MirFoundationValidationError::Identity)?;
    let generated_path = WirePath::root().field(2);
    let mut generated = HashMap::new();
    scoop_wire::allocation::try_reserve_map(&mut generated, hir_generated.len(), &generated_path)
        .map_err(MirFoundationValidationError::Resource)?;
    generated.extend(
        hir_generated
            .iter()
            .map(|record| (record.id(), record.key())),
    );
    scoop_wire::allocation::try_reserve_map(&mut generated, mir_generated.len(), &generated_path)
        .map_err(MirFoundationValidationError::Resource)?;
    generated.extend(
        mir_generated
            .iter()
            .map(|record| (record.id(), record.key())),
    );

    let hir_members = identities
        .records::<OdrMemberId, OdrMemberKey>(IdentityLayer::Hir, &WirePath::root().field(28))
        .map_err(MirFoundationValidationError::Identity)?;
    let mir_members = identities
        .records::<OdrMemberId, OdrMemberKey>(IdentityLayer::Mir, &WirePath::root().field(12))
        .map_err(MirFoundationValidationError::Identity)?;
    let member_path = WirePath::root().field(12);
    let mut members = HashMap::new();
    scoop_wire::allocation::try_reserve_map(&mut members, hir_members.len(), &member_path)
        .map_err(MirFoundationValidationError::Resource)?;
    members.extend(hir_members.iter().map(|record| (record.id(), record.key())));
    scoop_wire::allocation::try_reserve_map(&mut members, mir_members.len(), &member_path)
        .map_err(MirFoundationValidationError::Resource)?;
    members.extend(mir_members.iter().map(|record| (record.id(), record.key())));

    let signature_path = WirePath::root().field(7);
    let mut signatures_by_subject = HashMap::new();
    scoop_wire::allocation::try_reserve_map(
        &mut signatures_by_subject,
        signatures.len(),
        &signature_path,
    )
    .map_err(MirFoundationValidationError::Resource)?;
    for signature in signatures {
        signatures_by_subject
            .entry(signature.subject())
            .or_insert(signature);
    }

    let record_path = WirePath::root().field(10);
    let mut seen = HashSet::new();
    scoop_wire::allocation::try_reserve_set(&mut seen, records.len(), &record_path)
        .map_err(MirFoundationValidationError::Resource)?;
    for record in records {
        let application = record.application();
        if !seen.insert(application) {
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
        if record.mode() != registration.mode() {
            return Err(CallbackApplicationRelationError::ModeMismatch { application }.into());
        }
        let signature = signatures_by_subject.get(&record.managed_adapter()).ok_or(
            CallbackApplicationRelationError::MissingManagedSignature {
                application,
                subject: record.managed_adapter(),
            },
        )?;
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
            generated.get(&adapter),
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
    if let Some(application) = identity_records
        .iter()
        .map(CborIdentityRecord::id)
        .find(|application| !seen.contains(application))
    {
        return Err(CallbackApplicationRelationError::MissingRecord { application }.into());
    }
    Ok(())
}

fn managed_adapter_identity(
    subject: CallableSignatureSubject,
    members: &HashMap<OdrMemberId, &OdrMemberKey>,
) -> Option<PersistentGeneratedCallableId> {
    match subject {
        CallableSignatureSubject::Strong(CallableOwner::Generated(id)) => Some(id),
        CallableSignatureSubject::Odr(member) => {
            match members.get(&member.member())?.discriminator() {
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
    Resource(scoop_wire::WireError),
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
            Self::Resource(error) => error.fmt(formatter),
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
