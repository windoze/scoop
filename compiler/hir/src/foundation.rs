//! Arena-independent canonical projection of the HIR identity foundation.

use std::fmt;

use scoop_identity::{
    CallableApplicationKey, CallbackRegistrationKey, CborIdentityRecord, DefinitionOriginRecord,
    DefinitionOwnerAtom, DispatchSlotKey, EnumVariantFieldKey, EnumVariantIdentityKey,
    ExactTypeKey, ExportBindingKey, FieldIdentityKey, GeneratedCallableKey, GeneratedNominalKey,
    InitializationUnitKey, LocalBindingKey, LocalValueKey, OdrGroupId, OdrMemberId, OdrMemberKey,
    PersistentCallableApplicationId, PersistentCallbackRegistrationId, PersistentConstructorId,
    PersistentDispatchSlotId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentExtensionPropertyId,
    PersistentFieldId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentId,
    PersistentInitializationUnitId, PersistentLocalBindingId, PersistentLocalValueId,
    PersistentObjectValueId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentSourceContextId, PersistentTypeAliasId, PersistentTypeId, PropertyAccessorKey,
    SourceContextKey, SourceDeclarationKey, SourceIdentity, SourceNativeExternalContractRecord,
    SpecializationKey, StableIdentityOrderError, stable_topological_identity_delta_order,
};
use scoop_wire::{Encoder, WireEncode};

use crate::{NativeBoundaryTypeDefinitionRecord, SourceRecord, SourceRecordError};

mod wire;
pub use wire::{
    DecodedHirFoundation, DefinitionOriginValidationError, HirFoundationValidationError,
    NativeBoundaryShapeCoverageError, ValidatedHirFoundation,
};
mod counts;
mod imported;
mod projection;
mod strong_profile;
pub use counts::HirFoundationCounts;
pub use imported::{ImportedHirId, ImportedHirSet};
pub use strong_profile::{
    OdrFreeHirFoundation, OdrFreeHirFoundationError, OdrFreeHirFoundationProjectionError,
};

type TypeRecord = CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>;
type GenericTypeRecord = CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>;
type FunctionRecord = CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>;
type GenericFunctionRecord = CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey>;
type ConstructorRecord = CborIdentityRecord<PersistentConstructorId, SourceDeclarationKey>;
type PropertyRecord = CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>;
type ExtensionPropertyRecord =
    CborIdentityRecord<PersistentExtensionPropertyId, SourceDeclarationKey>;
type ObjectValueRecord = CborIdentityRecord<PersistentObjectValueId, SourceDeclarationKey>;
type TypeAliasRecord = CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>;
type PropertyAccessorRecord = CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type EnumVariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type EnumVariantFieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;
type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type ExportBindingRecord = CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>;
type CallableApplicationRecord =
    CborIdentityRecord<PersistentCallableApplicationId, CallableApplicationKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type DispatchSlotRecord = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;
type InitializationUnitRecord =
    CborIdentityRecord<PersistentInitializationUnitId, InitializationUnitKey>;
type SourceContextRecord = CborIdentityRecord<PersistentSourceContextId, SourceContextKey>;
type LocalBindingRecord = CborIdentityRecord<PersistentLocalBindingId, LocalBindingKey>;
type LocalValueRecord = CborIdentityRecord<PersistentLocalValueId, LocalValueKey>;
type CallbackRegistrationRecord =
    CborIdentityRecord<PersistentCallbackRegistrationId, CallbackRegistrationKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// Canonical, arena-independent HIR identity tables produced by lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHirFoundation {
    sources: Vec<SourceRecord>,
    types: Vec<TypeRecord>,
    generic_types: Vec<GenericTypeRecord>,
    functions: Vec<FunctionRecord>,
    generic_functions: Vec<GenericFunctionRecord>,
    constructors: Vec<ConstructorRecord>,
    properties: Vec<PropertyRecord>,
    extension_properties: Vec<ExtensionPropertyRecord>,
    object_values: Vec<ObjectValueRecord>,
    type_aliases: Vec<TypeAliasRecord>,
    property_accessors: Vec<PropertyAccessorRecord>,
    fields: Vec<FieldRecord>,
    enum_variants: Vec<EnumVariantRecord>,
    enum_variant_fields: Vec<EnumVariantFieldRecord>,
    exact_types: Vec<ExactTypeRecord>,
    export_bindings: Vec<ExportBindingRecord>,
    callable_applications: Vec<CallableApplicationRecord>,
    generated_callables: Vec<GeneratedCallableRecord>,
    generated_types: Vec<GeneratedTypeRecord>,
    dispatch_slots: Vec<DispatchSlotRecord>,
    initialization_units: Vec<InitializationUnitRecord>,
    source_contexts: Vec<SourceContextRecord>,
    local_bindings: Vec<LocalBindingRecord>,
    local_values: Vec<LocalValueRecord>,
    callback_registrations: Vec<CallbackRegistrationRecord>,
    source_native_contracts: Vec<SourceNativeExternalContractRecord>,
    odr_groups: Vec<OdrGroupRecord>,
    odr_members: Vec<OdrMemberRecord>,
    definition_origins: Vec<DefinitionOriginRecord>,
    native_boundary_types: Vec<NativeBoundaryTypeDefinitionRecord>,
}

macro_rules! simple_identity_setter {
    ($method:ident, $field:ident, $record:ty, $table:ident) => {
        pub fn $method(&mut self, records: Vec<$record>) -> Result<(), HirFoundationBuildError> {
            self.$field = sort_unique(records, HirFoundationTable::$table, CborIdentityRecord::id)?;
            Ok(())
        }
    };
}

impl CanonicalHirFoundation {
    pub const fn empty() -> Self {
        Self {
            sources: Vec::new(),
            types: Vec::new(),
            generic_types: Vec::new(),
            functions: Vec::new(),
            generic_functions: Vec::new(),
            constructors: Vec::new(),
            properties: Vec::new(),
            extension_properties: Vec::new(),
            object_values: Vec::new(),
            type_aliases: Vec::new(),
            property_accessors: Vec::new(),
            fields: Vec::new(),
            enum_variants: Vec::new(),
            enum_variant_fields: Vec::new(),
            exact_types: Vec::new(),
            export_bindings: Vec::new(),
            callable_applications: Vec::new(),
            generated_callables: Vec::new(),
            generated_types: Vec::new(),
            dispatch_slots: Vec::new(),
            initialization_units: Vec::new(),
            source_contexts: Vec::new(),
            local_bindings: Vec::new(),
            local_values: Vec::new(),
            callback_registrations: Vec::new(),
            source_native_contracts: Vec::new(),
            odr_groups: Vec::new(),
            odr_members: Vec::new(),
            definition_origins: Vec::new(),
            native_boundary_types: Vec::new(),
        }
    }

    pub(crate) fn function_id_by_bytes(&self, bytes: &[u8; 32]) -> Option<PersistentFunctionId> {
        self.functions
            .iter()
            .map(CborIdentityRecord::id)
            .find(|id| id.as_array() == bytes)
    }

    pub(crate) fn export_binding_id_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<PersistentExportBindingId> {
        self.export_bindings
            .iter()
            .map(CborIdentityRecord::id)
            .find(|id| id.as_array() == bytes)
    }

    pub(crate) fn export_binding_key(
        &self,
        id: PersistentExportBindingId,
    ) -> Option<&ExportBindingKey> {
        self.export_bindings
            .iter()
            .find(|record| record.id() == id)
            .map(CborIdentityRecord::key)
    }

    pub(crate) fn source_type_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentTypeId, &SourceDeclarationKey)> {
        self.types.iter().find_map(|record| {
            (record.id().as_array() == bytes).then(|| (record.id(), record.key()))
        })
    }

    pub(crate) fn exact_type_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentExactTypeId, &ExactTypeKey)> {
        self.exact_types.iter().find_map(|record| {
            (record.id().as_array() == bytes).then(|| (record.id(), record.key()))
        })
    }

    pub(crate) fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
        self.exact_types
            .iter()
            .find(|record| record.id() == id)
            .map(CborIdentityRecord::key)
    }

    pub(crate) fn generic_type_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentGenericTypeId, &SourceDeclarationKey)> {
        self.generic_types.iter().find_map(|record| {
            (record.id().as_array() == bytes).then(|| (record.id(), record.key()))
        })
    }

    pub(crate) fn function_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentFunctionId, &SourceDeclarationKey)> {
        self.functions.iter().find_map(|record| {
            (record.id().as_array() == bytes).then(|| (record.id(), record.key()))
        })
    }

    pub(crate) fn generic_function_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentGenericFunctionId, &SourceDeclarationKey)> {
        self.generic_functions.iter().find_map(|record| {
            (record.id().as_array() == bytes).then(|| (record.id(), record.key()))
        })
    }

    pub(crate) fn definition_origin(
        &self,
        subject: scoop_identity::DefinitionOriginSubject,
    ) -> Option<&DefinitionOriginRecord> {
        self.definition_origins
            .binary_search_by(|record| record.subject().compare_sort_key(subject))
            .ok()
            .map(|index| &self.definition_origins[index])
    }

    pub(crate) fn generic_type_key(
        &self,
        id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        self.generic_types
            .iter()
            .find(|record| record.id() == id)
            .map(CborIdentityRecord::key)
    }

    pub(crate) fn enum_variant_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentEnumVariantId, &EnumVariantIdentityKey)> {
        self.enum_variants.iter().find_map(|record| {
            (record.id().as_array() == bytes).then(|| (record.id(), record.key()))
        })
    }

    pub(crate) fn enum_variant_field_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentEnumVariantFieldId, &EnumVariantFieldKey)> {
        self.enum_variant_fields.iter().find_map(|record| {
            (record.id().as_array() == bytes).then(|| (record.id(), record.key()))
        })
    }

    pub(crate) fn enum_variant_field_count(&self, variant: PersistentEnumVariantId) -> usize {
        self.enum_variant_fields
            .iter()
            .filter(|record| record.key().variant() == variant)
            .count()
    }

    pub fn set_sources(
        &mut self,
        mut records: Vec<SourceRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        records.sort_by(|left, right| left.identity().cmp(right.identity()));
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].identity() == pair[1].identity())
        {
            return Err(HirFoundationBuildError::DuplicateSourceIdentity(
                pair[0].identity().clone(),
            ));
        }
        self.sources = records;
        Ok(())
    }

    pub fn set_types(&mut self, records: Vec<TypeRecord>) -> Result<(), HirFoundationBuildError> {
        self.types = order_declarations(records, HirFoundationTable::Type, |owner| match owner {
            DefinitionOwnerAtom::Type(id) => Some(*id),
            _ => None,
        })?;
        Ok(())
    }

    pub fn set_generic_types(
        &mut self,
        records: Vec<GenericTypeRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.generic_types = order_declarations(
            records,
            HirFoundationTable::GenericType,
            |owner| match owner {
                DefinitionOwnerAtom::GenericType(id) => Some(*id),
                _ => None,
            },
        )?;
        Ok(())
    }

    pub fn set_functions(
        &mut self,
        records: Vec<FunctionRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.functions =
            order_declarations(records, HirFoundationTable::Function, |owner| match owner {
                DefinitionOwnerAtom::Function(id) => Some(*id),
                _ => None,
            })?;
        Ok(())
    }

    pub fn set_generic_functions(
        &mut self,
        records: Vec<GenericFunctionRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.generic_functions = order_declarations(
            records,
            HirFoundationTable::GenericFunction,
            |owner| match owner {
                DefinitionOwnerAtom::GenericFunction(id) => Some(*id),
                _ => None,
            },
        )?;
        Ok(())
    }

    pub fn set_constructors(
        &mut self,
        records: Vec<ConstructorRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.constructors = order_declarations(
            records,
            HirFoundationTable::Constructor,
            |owner| match owner {
                DefinitionOwnerAtom::Constructor(id) => Some(*id),
                _ => None,
            },
        )?;
        Ok(())
    }

    pub fn set_properties(
        &mut self,
        records: Vec<PropertyRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.properties =
            order_declarations(records, HirFoundationTable::Property, |owner| match owner {
                DefinitionOwnerAtom::Property(id) => Some(*id),
                _ => None,
            })?;
        Ok(())
    }

    pub fn set_extension_properties(
        &mut self,
        records: Vec<ExtensionPropertyRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.extension_properties = order_declarations(
            records,
            HirFoundationTable::ExtensionProperty,
            |owner| match owner {
                DefinitionOwnerAtom::ExtensionProperty(id) => Some(*id),
                _ => None,
            },
        )?;
        Ok(())
    }

    simple_identity_setter!(
        set_object_values,
        object_values,
        ObjectValueRecord,
        ObjectValue
    );
    simple_identity_setter!(set_type_aliases, type_aliases, TypeAliasRecord, TypeAlias);
    simple_identity_setter!(
        set_property_accessors,
        property_accessors,
        PropertyAccessorRecord,
        PropertyAccessor
    );
    simple_identity_setter!(set_fields, fields, FieldRecord, Field);
    simple_identity_setter!(
        set_enum_variants,
        enum_variants,
        EnumVariantRecord,
        EnumVariant
    );
    simple_identity_setter!(
        set_enum_variant_fields,
        enum_variant_fields,
        EnumVariantFieldRecord,
        EnumVariantField
    );

    pub fn set_exact_types(
        &mut self,
        records: Vec<ExactTypeRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.exact_types =
            stable_topological_identity_delta_order(records, CborIdentityRecord::id, |record| {
                record.key().exact_type_dependencies()
            })
            .map_err(|error| order_error(HirFoundationTable::ExactType, error))?;
        Ok(())
    }

    simple_identity_setter!(
        set_export_bindings,
        export_bindings,
        ExportBindingRecord,
        ExportBinding
    );

    pub fn set_callable_applications(
        &mut self,
        records: Vec<CallableApplicationRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.callable_applications =
            stable_topological_identity_delta_order(records, CborIdentityRecord::id, |record| {
                record.key().callable_application_dependencies()
            })
            .map_err(|error| order_error(HirFoundationTable::CallableApplication, error))?;
        Ok(())
    }

    pub fn set_generated_callables(
        &mut self,
        records: Vec<GeneratedCallableRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.generated_callables =
            stable_topological_identity_delta_order(records, CborIdentityRecord::id, |record| {
                record.key().generated_callable_dependencies()
            })
            .map_err(|error| order_error(HirFoundationTable::GeneratedCallable, error))?;
        Ok(())
    }

    simple_identity_setter!(
        set_generated_types,
        generated_types,
        GeneratedTypeRecord,
        GeneratedType
    );
    simple_identity_setter!(
        set_dispatch_slots,
        dispatch_slots,
        DispatchSlotRecord,
        DispatchSlot
    );
    simple_identity_setter!(
        set_initialization_units,
        initialization_units,
        InitializationUnitRecord,
        InitializationUnit
    );
    simple_identity_setter!(
        set_source_contexts,
        source_contexts,
        SourceContextRecord,
        SourceContext
    );
    simple_identity_setter!(
        set_local_bindings,
        local_bindings,
        LocalBindingRecord,
        LocalBinding
    );
    simple_identity_setter!(set_local_values, local_values, LocalValueRecord, LocalValue);
    simple_identity_setter!(
        set_callback_registrations,
        callback_registrations,
        CallbackRegistrationRecord,
        CallbackRegistration
    );

    pub fn set_source_native_contracts(
        &mut self,
        records: Vec<SourceNativeExternalContractRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.source_native_contracts = sort_unique(
            records,
            HirFoundationTable::SourceNativeContract,
            SourceNativeExternalContractRecord::id,
        )?;
        Ok(())
    }

    simple_identity_setter!(set_odr_groups, odr_groups, OdrGroupRecord, OdrGroup);
    simple_identity_setter!(set_odr_members, odr_members, OdrMemberRecord, OdrMember);

    pub fn set_definition_origins(
        &mut self,
        mut records: Vec<DefinitionOriginRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        records.sort_by(|left, right| left.subject().compare_sort_key(right.subject()));
        if let Some(pair) = records.windows(2).find(|pair| {
            pair[0]
                .subject()
                .compare_sort_key(pair[1].subject())
                .is_eq()
        }) {
            return Err(duplicate_subject(
                HirFoundationTable::DefinitionOrigin,
                pair[0].subject().kind_tag(),
                pair[0].subject().raw_id(),
            ));
        }
        self.definition_origins = records;
        Ok(())
    }

    pub fn set_native_boundary_types(
        &mut self,
        mut records: Vec<NativeBoundaryTypeDefinitionRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        records.sort_by(|left, right| left.owner().compare_sort_key(right.owner()));
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].owner().compare_sort_key(pair[1].owner()).is_eq())
        {
            return Err(duplicate_subject(
                HirFoundationTable::NativeBoundaryType,
                pair[0].owner().kind_tag(),
                pair[0].owner().raw_id(),
            ));
        }
        self.native_boundary_types = records;
        Ok(())
    }
}

impl WireEncode for CanonicalHirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(30)?;
        encode_table_field(encoder, 1, &self.sources)?;
        encode_table_field(encoder, 2, &self.types)?;
        encode_table_field(encoder, 3, &self.generic_types)?;
        encode_table_field(encoder, 4, &self.functions)?;
        encode_table_field(encoder, 5, &self.generic_functions)?;
        encode_table_field(encoder, 6, &self.constructors)?;
        encode_table_field(encoder, 7, &self.properties)?;
        encode_table_field(encoder, 8, &self.extension_properties)?;
        encode_table_field(encoder, 9, &self.object_values)?;
        encode_table_field(encoder, 10, &self.type_aliases)?;
        encode_table_field(encoder, 11, &self.property_accessors)?;
        encode_table_field(encoder, 12, &self.fields)?;
        encode_table_field(encoder, 13, &self.enum_variants)?;
        encode_table_field(encoder, 14, &self.enum_variant_fields)?;
        encode_table_field(encoder, 15, &self.exact_types)?;
        encode_table_field(encoder, 16, &self.export_bindings)?;
        encode_table_field(encoder, 17, &self.callable_applications)?;
        encode_table_field(encoder, 18, &self.generated_callables)?;
        encode_table_field(encoder, 19, &self.generated_types)?;
        encode_table_field(encoder, 20, &self.dispatch_slots)?;
        encode_table_field(encoder, 21, &self.initialization_units)?;
        encode_table_field(encoder, 22, &self.source_contexts)?;
        encode_table_field(encoder, 23, &self.local_bindings)?;
        encode_table_field(encoder, 24, &self.local_values)?;
        encode_table_field(encoder, 25, &self.callback_registrations)?;
        encode_table_field(encoder, 26, &self.source_native_contracts)?;
        encode_table_field(encoder, 27, &self.odr_groups)?;
        encode_table_field(encoder, 28, &self.odr_members)?;
        encode_table_field(encoder, 29, &self.definition_origins)?;
        encode_table_field(encoder, 30, &self.native_boundary_types)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirFoundationTable {
    Source,
    Type,
    GenericType,
    Function,
    GenericFunction,
    Constructor,
    Property,
    ExtensionProperty,
    ObjectValue,
    TypeAlias,
    PropertyAccessor,
    Field,
    EnumVariant,
    EnumVariantField,
    ExactType,
    ExportBinding,
    CallableApplication,
    GeneratedCallable,
    GeneratedType,
    DispatchSlot,
    InitializationUnit,
    SourceContext,
    LocalBinding,
    LocalValue,
    CallbackRegistration,
    SourceNativeContract,
    OdrGroup,
    OdrMember,
    DefinitionOrigin,
    NativeBoundaryType,
}

impl HirFoundationTable {
    const fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Type => "type",
            Self::GenericType => "generic type",
            Self::Function => "function",
            Self::GenericFunction => "generic function",
            Self::Constructor => "constructor",
            Self::Property => "property",
            Self::ExtensionProperty => "extension property",
            Self::ObjectValue => "object value",
            Self::TypeAlias => "type alias",
            Self::PropertyAccessor => "property accessor",
            Self::Field => "field",
            Self::EnumVariant => "enum variant",
            Self::EnumVariantField => "enum variant field",
            Self::ExactType => "exact type",
            Self::ExportBinding => "export binding",
            Self::CallableApplication => "callable application",
            Self::GeneratedCallable => "generated callable",
            Self::GeneratedType => "generated type",
            Self::DispatchSlot => "dispatch slot",
            Self::InitializationUnit => "initialization unit",
            Self::SourceContext => "source context",
            Self::LocalBinding => "local binding",
            Self::LocalValue => "local value",
            Self::CallbackRegistration => "callback registration",
            Self::SourceNativeContract => "source native contract",
            Self::OdrGroup => "ODR group",
            Self::OdrMember => "ODR member",
            Self::DefinitionOrigin => "definition origin",
            Self::NativeBoundaryType => "native boundary type",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirFoundationOrderIssue {
    Duplicate([u8; 32]),
    MissingDependency {
        identity: [u8; 32],
        dependency: [u8; 32],
    },
    Cycle([u8; 32]),
    NonCanonical {
        position: usize,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    Inconsistent([u8; 32]),
}

#[derive(Debug)]
pub enum HirFoundationBuildError {
    DependencyOrder {
        table: HirFoundationTable,
        issue: HirFoundationOrderIssue,
    },
    DuplicateIdentity {
        table: HirFoundationTable,
        identity: [u8; 32],
    },
    DuplicateSourceIdentity(SourceIdentity),
    DuplicateSubject {
        table: HirFoundationTable,
        subject_tag: u8,
        subject: [u8; 32],
    },
    SourceRecord {
        source: SourceIdentity,
        error: SourceRecordError,
    },
    UnknownDefinitionSource {
        source: SourceIdentity,
        subject_tag: u8,
        subject: [u8; 32],
    },
    IdentityDerivation {
        table: HirFoundationTable,
        reason: String,
    },
}

impl fmt::Display for HirFoundationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DependencyOrder { table, .. } => {
                write!(
                    formatter,
                    "{} identity dependency order is invalid",
                    table.name()
                )
            }
            Self::DuplicateIdentity { table, identity } => write!(
                formatter,
                "duplicate {} identity {}",
                table.name(),
                HexIdentity(identity)
            ),
            Self::DuplicateSourceIdentity(identity) => {
                write!(formatter, "duplicate source identity {identity:?}")
            }
            Self::DuplicateSubject {
                table,
                subject_tag,
                subject,
            } => write!(
                formatter,
                "duplicate {} subject {subject_tag}:{}",
                table.name(),
                HexIdentity(subject)
            ),
            Self::SourceRecord { source, error } => {
                write!(formatter, "cannot project source {source:?}: {error}")
            }
            Self::UnknownDefinitionSource {
                source,
                subject_tag,
                subject,
            } => write!(
                formatter,
                "definition subject {subject_tag}:{} refers to unknown source {source:?}",
                HexIdentity(subject)
            ),
            Self::IdentityDerivation { table, reason } => {
                write!(
                    formatter,
                    "cannot derive {} identity: {reason}",
                    table.name()
                )
            }
        }
    }
}

impl std::error::Error for HirFoundationBuildError {}

fn order_declarations<I: PersistentId>(
    records: Vec<CborIdentityRecord<I, SourceDeclarationKey>>,
    table: HirFoundationTable,
    dependency: impl Fn(&DefinitionOwnerAtom) -> Option<I>,
) -> Result<Vec<CborIdentityRecord<I, SourceDeclarationKey>>, HirFoundationBuildError>
where
    SourceDeclarationKey: scoop_identity::CborIdentityKey<I>,
{
    stable_topological_identity_delta_order(records, CborIdentityRecord::id, |record| {
        record
            .key()
            .owners()
            .owners()
            .iter()
            .filter_map(&dependency)
            .collect()
    })
    .map_err(|error| order_error(table, error))
}

fn sort_unique<T, I: PersistentId>(
    mut records: Vec<T>,
    table: HirFoundationTable,
    id_of: impl Fn(&T) -> I,
) -> Result<Vec<T>, HirFoundationBuildError> {
    records.sort_by_key(|record| id_of(record));
    if let Some(pair) = records
        .windows(2)
        .find(|pair| id_of(&pair[0]) == id_of(&pair[1]))
    {
        return Err(HirFoundationBuildError::DuplicateIdentity {
            table,
            identity: *id_of(&pair[0]).as_array(),
        });
    }
    Ok(records)
}

fn order_error<I: PersistentId>(
    table: HirFoundationTable,
    error: StableIdentityOrderError<I>,
) -> HirFoundationBuildError {
    let issue = match error {
        StableIdentityOrderError::DuplicateIdentity(id) => {
            HirFoundationOrderIssue::Duplicate(*id.as_array())
        }
        StableIdentityOrderError::MissingDependency {
            identity,
            dependency,
        } => HirFoundationOrderIssue::MissingDependency {
            identity: *identity.as_array(),
            dependency: *dependency.as_array(),
        },
        StableIdentityOrderError::Cycle { first } => {
            HirFoundationOrderIssue::Cycle(*first.as_array())
        }
        StableIdentityOrderError::NonCanonicalOrder {
            position,
            expected,
            actual,
        } => HirFoundationOrderIssue::NonCanonical {
            position,
            expected: *expected.as_array(),
            actual: *actual.as_array(),
        },
        StableIdentityOrderError::InconsistentGraph(id) => {
            HirFoundationOrderIssue::Inconsistent(*id.as_array())
        }
    };
    HirFoundationBuildError::DependencyOrder { table, issue }
}

fn duplicate_subject(
    table: HirFoundationTable,
    subject_tag: u8,
    subject: [u8; 32],
) -> HirFoundationBuildError {
    HirFoundationBuildError::DuplicateSubject {
        table,
        subject_tag,
        subject,
    }
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
