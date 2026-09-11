//! Arena-independent canonical projection of the MIR identity foundation.

use std::fmt;

use scoop_identity::{
    CallbackApplicationKey, CborIdentityRecord, EnumVariantFieldKey, EnumVariantIdentityKey,
    ExactTypeKey, FieldIdentityKey, GeneratedCallableKey, GeneratedNominalKey, LocalValueKey,
    OdrGroupId, OdrMemberId, OdrMemberKey, PersistentCallbackApplicationId,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentFieldId, PersistentGeneratedCallableId, PersistentId, PersistentLocalValueId,
    PersistentTypeId, SpecializationKey, StableIdentityOrderError,
    stable_topological_identity_delta_order,
};
use scoop_wire::{Encoder, WireEncode};

use crate::{
    CallableSignatureRecord, CallbackApplicationRecord, MirCallableSignatureRelationError,
    MirCallableSignatures,
};

mod wire;
pub use wire::ValidatedMirFoundationWire;
mod projection;

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type EnumVariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type EnumVariantFieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;
type LocalValueRecord = CborIdentityRecord<PersistentLocalValueId, LocalValueKey>;
type CallbackApplicationIdentityRecord =
    CborIdentityRecord<PersistentCallbackApplicationId, CallbackApplicationKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// Canonical, arena-independent MIR identity tables produced by lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMirFoundation {
    exact_types: Vec<ExactTypeRecord>,
    generated_callables: Vec<GeneratedCallableRecord>,
    generated_types: Vec<GeneratedTypeRecord>,
    fields: Vec<FieldRecord>,
    enum_variants: Vec<EnumVariantRecord>,
    enum_variant_fields: Vec<EnumVariantFieldRecord>,
    callable_signatures: Vec<CallableSignatureRecord>,
    local_values: Vec<LocalValueRecord>,
    callback_applications: Vec<CallbackApplicationIdentityRecord>,
    callback_application_records: Vec<CallbackApplicationRecord>,
    odr_groups: Vec<OdrGroupRecord>,
    odr_members: Vec<OdrMemberRecord>,
}

impl CanonicalMirFoundation {
    pub const fn empty() -> Self {
        Self {
            exact_types: Vec::new(),
            generated_callables: Vec::new(),
            generated_types: Vec::new(),
            fields: Vec::new(),
            enum_variants: Vec::new(),
            enum_variant_fields: Vec::new(),
            callable_signatures: Vec::new(),
            local_values: Vec::new(),
            callback_applications: Vec::new(),
            callback_application_records: Vec::new(),
            odr_groups: Vec::new(),
            odr_members: Vec::new(),
        }
    }

    pub fn counts(&self) -> MirFoundationCounts {
        MirFoundationCounts {
            exact_types: self.exact_types.len(),
            generated_callables: self.generated_callables.len(),
            generated_types: self.generated_types.len(),
            fields: self.fields.len(),
            enum_variants: self.enum_variants.len(),
            enum_variant_fields: self.enum_variant_fields.len(),
            callable_signatures: self.callable_signatures.len(),
            local_values: self.local_values.len(),
            callback_applications: self.callback_applications.len(),
            callback_application_records: self.callback_application_records.len(),
            odr_groups: self.odr_groups.len(),
            odr_members: self.odr_members.len(),
        }
    }

    pub fn set_exact_types(
        &mut self,
        records: Vec<ExactTypeRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.exact_types =
            stable_topological_identity_delta_order(records, CborIdentityRecord::id, |record| {
                record.key().exact_type_dependencies()
            })
            .map_err(MirFoundationBuildError::ExactTypeOrder)?;
        Ok(())
    }

    pub fn set_generated_callables(
        &mut self,
        records: Vec<GeneratedCallableRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.generated_callables =
            stable_topological_identity_delta_order(records, CborIdentityRecord::id, |record| {
                record.key().generated_callable_dependencies()
            })
            .map_err(MirFoundationBuildError::GeneratedCallableOrder)?;
        Ok(())
    }

    pub fn set_generated_types(
        &mut self,
        records: Vec<GeneratedTypeRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.generated_types = sort_unique(
            records,
            MirFoundationTable::GeneratedType,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_fields(&mut self, records: Vec<FieldRecord>) -> Result<(), MirFoundationBuildError> {
        self.fields = sort_unique(records, MirFoundationTable::Field, CborIdentityRecord::id)?;
        Ok(())
    }

    pub fn set_enum_variants(
        &mut self,
        records: Vec<EnumVariantRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.enum_variants = sort_unique(
            records,
            MirFoundationTable::EnumVariant,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_enum_variant_fields(
        &mut self,
        records: Vec<EnumVariantFieldRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.enum_variant_fields = sort_unique(
            records,
            MirFoundationTable::EnumVariantField,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_callable_signatures(
        &mut self,
        records: Vec<CallableSignatureRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.callable_signatures = MirCallableSignatures::checked(records)
            .map_err(|error| match error {
                MirCallableSignatureRelationError::DuplicateSubject { subject, .. } => {
                    MirFoundationBuildError::DuplicateCallableSignature {
                        subject_tag: subject.kind_tag(),
                        subject: subject.raw_id(),
                    }
                }
            })?
            .into_records();
        Ok(())
    }

    pub fn set_local_values(
        &mut self,
        records: Vec<LocalValueRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.local_values = sort_unique(
            records,
            MirFoundationTable::LocalValue,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_callback_applications(
        &mut self,
        records: Vec<CallbackApplicationIdentityRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.callback_applications = sort_unique(
            records,
            MirFoundationTable::CallbackApplication,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_callback_application_records(
        &mut self,
        records: Vec<CallbackApplicationRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.callback_application_records = sort_unique(
            records,
            MirFoundationTable::CallbackApplicationRecord,
            CallbackApplicationRecord::application,
        )?;
        Ok(())
    }

    pub fn set_odr_groups(
        &mut self,
        records: Vec<OdrGroupRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.odr_groups = sort_unique(
            records,
            MirFoundationTable::OdrGroup,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_odr_members(
        &mut self,
        records: Vec<OdrMemberRecord>,
    ) -> Result<(), MirFoundationBuildError> {
        self.odr_members = sort_unique(
            records,
            MirFoundationTable::OdrMember,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirFoundationCounts {
    pub exact_types: usize,
    pub generated_callables: usize,
    pub generated_types: usize,
    pub fields: usize,
    pub enum_variants: usize,
    pub enum_variant_fields: usize,
    pub callable_signatures: usize,
    pub local_values: usize,
    pub callback_applications: usize,
    pub callback_application_records: usize,
    pub odr_groups: usize,
    pub odr_members: usize,
}

impl WireEncode for CanonicalMirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_table_field(encoder, 1, &self.exact_types)?;
        encode_table_field(encoder, 2, &self.generated_callables)?;
        encode_table_field(encoder, 3, &self.generated_types)?;
        encode_table_field(encoder, 4, &self.fields)?;
        encode_table_field(encoder, 5, &self.enum_variants)?;
        encode_table_field(encoder, 6, &self.enum_variant_fields)?;
        encode_table_field(encoder, 7, &self.callable_signatures)?;
        encode_table_field(encoder, 8, &self.local_values)?;
        encode_table_field(encoder, 9, &self.callback_applications)?;
        encode_table_field(encoder, 10, &self.callback_application_records)?;
        encode_table_field(encoder, 11, &self.odr_groups)?;
        encode_table_field(encoder, 12, &self.odr_members)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirFoundationTable {
    GeneratedType,
    Field,
    EnumVariant,
    EnumVariantField,
    LocalValue,
    CallbackApplication,
    CallbackApplicationRecord,
    OdrGroup,
    OdrMember,
}

impl MirFoundationTable {
    const fn name(self) -> &'static str {
        match self {
            Self::GeneratedType => "generated type",
            Self::Field => "field",
            Self::EnumVariant => "enum variant",
            Self::EnumVariantField => "enum variant field",
            Self::LocalValue => "local value",
            Self::CallbackApplication => "callback application",
            Self::CallbackApplicationRecord => "callback application record",
            Self::OdrGroup => "ODR group",
            Self::OdrMember => "ODR member",
        }
    }
}

#[derive(Debug)]
pub enum MirFoundationBuildError {
    InvalidModule(Box<crate::MirValidationError>),
    ExactTypeOrder(StableIdentityOrderError<PersistentExactTypeId>),
    GeneratedCallableOrder(StableIdentityOrderError<PersistentGeneratedCallableId>),
    DuplicateIdentity {
        table: MirFoundationTable,
        identity: [u8; 32],
    },
    IdentityCollision {
        table: MirFoundationTable,
        identity: [u8; 32],
    },
    DuplicateCallableSignature {
        subject_tag: u8,
        subject: [u8; 32],
    },
}

impl fmt::Display for MirFoundationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModule(error) => error.fmt(formatter),
            Self::ExactTypeOrder(error) => error.fmt(formatter),
            Self::GeneratedCallableOrder(error) => error.fmt(formatter),
            Self::DuplicateIdentity { table, identity } => write!(
                formatter,
                "duplicate {} identity {}",
                table.name(),
                HexIdentity(identity)
            ),
            Self::IdentityCollision { table, identity } => write!(
                formatter,
                "conflicting {} identity {}",
                table.name(),
                HexIdentity(identity)
            ),
            Self::DuplicateCallableSignature {
                subject_tag,
                subject,
            } => write!(
                formatter,
                "duplicate callable signature subject {subject_tag}:{}",
                HexIdentity(subject)
            ),
        }
    }
}

impl std::error::Error for MirFoundationBuildError {}

fn sort_unique<T, I: PersistentId>(
    mut records: Vec<T>,
    table: MirFoundationTable,
    id_of: impl Fn(&T) -> I,
) -> Result<Vec<T>, MirFoundationBuildError> {
    records.sort_by_key(|record| id_of(record));
    if let Some(pair) = records
        .windows(2)
        .find(|pair| id_of(&pair[0]) == id_of(&pair[1]))
    {
        return Err(MirFoundationBuildError::DuplicateIdentity {
            table,
            identity: *id_of(&pair[0]).as_array(),
        });
    }
    Ok(records)
}

fn encode_table_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

struct HexIdentity<'a>(&'a [u8; 32]);

impl fmt::Display for HexIdentity<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
