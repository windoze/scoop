use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DefinitionOriginRecordResolutionError,
    IdentityLayer, IdentityReferenceError, IdentityValidationError, PersistentId, SourceContextKey,
    SourceNativeExternalResolutionError, ValidatedIdentityGraph,
};
use scoop_wire::{WireEncode, encode};

use super::*;
use crate::{NativeBoundaryResolutionError, NativeBoundaryResolver, SourceRecordValidationError};

mod origins;
pub use origins::DefinitionOriginValidationError;
mod native_boundary;
pub use native_boundary::NativeBoundaryShapeCoverageError;

/// HIR foundation whose complete local structure has been reconstructed from
/// validated persistent identities.
///
/// Cross-layer native-boundary closure and semantic-world import remain later
/// proofs and are intentionally not implied by this type.
pub struct ValidatedHirFoundation {
    canonical: CanonicalHirFoundation,
}

impl ValidatedHirFoundation {
    pub fn counts(&self) -> HirFoundationCounts {
        self.canonical.counts()
    }
}

impl WireEncode for ValidatedHirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

impl DecodedHirFoundation {
    /// Validates every HIR-local structural relation and only then publishes
    /// the immutable validation proof.
    pub fn validate(
        self,
        coordinate: &ConeCoordinate,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ValidatedHirFoundation, HirFoundationValidationError> {
        validate_foundation(self, coordinate, identities)
    }
}

fn validate_foundation(
    foundation: DecodedHirFoundation,
    coordinate: &ConeCoordinate,
    identities: &mut ValidatedIdentityGraph,
) -> Result<ValidatedHirFoundation, HirFoundationValidationError> {
    let original = encode(&foundation).map_err(HirFoundationValidationError::WireEncode)?;
    let artifact = coordinate
        .identity()
        .map_err(HirFoundationValidationError::CoordinateIdentity)?;
    let DecodedHirFoundationWire {
        sources,
        types: _,
        generic_types: _,
        functions: _,
        generic_functions: _,
        constructors: _,
        properties: _,
        extension_properties: _,
        object_values: _,
        type_aliases: _,
        property_accessors: _,
        fields: _,
        enum_variants: _,
        enum_variant_fields: _,
        exact_types: _,
        export_bindings: _,
        callable_applications: _,
        generated_callables: _,
        generated_types: _,
        dispatch_slots: _,
        initialization_units: _,
        source_contexts: _,
        local_bindings: _,
        local_values: _,
        callback_registrations: _,
        source_native_contracts,
        odr_groups: _,
        odr_members: _,
        definition_origins,
        native_boundary_types,
    } = foundation.decoded;

    let mut validated_sources = Vec::new();
    validated_sources
        .try_reserve_exact(sources.len())
        .map_err(|_| HirFoundationValidationError::Allocation)?;
    for (index, source) in sources.into_iter().enumerate() {
        validated_sources.push(
            source
                .validate(coordinate, None)
                .map_err(|error| HirFoundationValidationError::SourceRecord { index, error })?,
        );
    }

    macro_rules! records {
        ($id:ty, $key:ty) => {
            identities
                .records::<$id, $key>(IdentityLayer::Hir)
                .map_err(HirFoundationValidationError::Identity)?
        };
    }

    let types: Vec<TypeRecord> = records!(PersistentTypeId, SourceDeclarationKey);
    let generic_types: Vec<GenericTypeRecord> =
        records!(PersistentGenericTypeId, SourceDeclarationKey);
    let functions: Vec<FunctionRecord> = records!(PersistentFunctionId, SourceDeclarationKey);
    let generic_functions: Vec<GenericFunctionRecord> =
        records!(PersistentGenericFunctionId, SourceDeclarationKey);
    let constructors: Vec<ConstructorRecord> =
        records!(PersistentConstructorId, SourceDeclarationKey);
    let properties: Vec<PropertyRecord> = records!(PersistentPropertyId, SourceDeclarationKey);
    let extension_properties: Vec<ExtensionPropertyRecord> =
        records!(PersistentExtensionPropertyId, SourceDeclarationKey);
    let object_values: Vec<ObjectValueRecord> =
        records!(PersistentObjectValueId, SourceDeclarationKey);
    let type_aliases: Vec<TypeAliasRecord> = records!(PersistentTypeAliasId, SourceDeclarationKey);
    let property_accessors: Vec<PropertyAccessorRecord> =
        records!(PersistentPropertyAccessorId, PropertyAccessorKey);
    let fields: Vec<FieldRecord> = records!(PersistentFieldId, FieldIdentityKey);
    let enum_variants: Vec<EnumVariantRecord> =
        records!(PersistentEnumVariantId, EnumVariantIdentityKey);
    let enum_variant_fields: Vec<EnumVariantFieldRecord> =
        records!(PersistentEnumVariantFieldId, EnumVariantFieldKey);
    let exact_types: Vec<ExactTypeRecord> = records!(PersistentExactTypeId, ExactTypeKey);
    let export_bindings: Vec<ExportBindingRecord> =
        records!(PersistentExportBindingId, ExportBindingKey);
    let callable_applications: Vec<CallableApplicationRecord> =
        records!(PersistentCallableApplicationId, CallableApplicationKey);
    let generated_callables: Vec<GeneratedCallableRecord> =
        records!(PersistentGeneratedCallableId, GeneratedCallableKey);
    let generated_types: Vec<GeneratedTypeRecord> = records!(PersistentTypeId, GeneratedNominalKey);
    let dispatch_slots: Vec<DispatchSlotRecord> =
        records!(PersistentDispatchSlotId, DispatchSlotKey);
    let initialization_units: Vec<InitializationUnitRecord> =
        records!(PersistentInitializationUnitId, InitializationUnitKey);
    let source_contexts: Vec<SourceContextRecord> =
        records!(PersistentSourceContextId, SourceContextKey);
    let local_bindings: Vec<LocalBindingRecord> =
        records!(PersistentLocalBindingId, LocalBindingKey);
    let local_values: Vec<LocalValueRecord> = records!(PersistentLocalValueId, LocalValueKey);
    let callback_registrations: Vec<CallbackRegistrationRecord> =
        records!(PersistentCallbackRegistrationId, CallbackRegistrationKey);
    let odr_groups: Vec<OdrGroupRecord> = records!(OdrGroupId, SpecializationKey);
    let odr_members: Vec<OdrMemberRecord> = records!(OdrMemberId, OdrMemberKey);

    validate_declaration_ownership(
        artifact,
        &types,
        &generic_types,
        &functions,
        &generic_functions,
        &constructors,
        &properties,
        &extension_properties,
        &object_values,
        &type_aliases,
    )?;
    validate_source_contexts(&source_contexts, &validated_sources)?;

    let mut contracts = Vec::new();
    contracts
        .try_reserve_exact(source_native_contracts.len())
        .map_err(|_| HirFoundationValidationError::Allocation)?;
    for (index, contract) in source_native_contracts.into_iter().enumerate() {
        contracts.push(contract.resolve(identities).map_err(|error| {
            HirFoundationValidationError::SourceNativeContract { index, error }
        })?);
    }

    let mut origins = Vec::new();
    origins
        .try_reserve_exact(definition_origins.len())
        .map_err(|_| HirFoundationValidationError::Allocation)?;
    for (index, origin) in definition_origins.into_iter().enumerate() {
        origins.push(
            origin
                .resolve(identities)
                .map_err(|error| HirFoundationValidationError::DefinitionOrigin { index, error })?,
        );
    }
    origins::validate(
        artifact,
        &validated_sources,
        &types,
        &generic_types,
        &functions,
        &generic_functions,
        &constructors,
        &properties,
        &extension_properties,
        &type_aliases,
        &property_accessors,
        &fields,
        &enum_variants,
        &enum_variant_fields,
        &generated_callables,
        &initialization_units,
        &local_bindings,
        &local_values,
        &callback_registrations,
        &contracts,
        &origins,
    )?;

    let mut boundary_types = Vec::new();
    boundary_types
        .try_reserve_exact(native_boundary_types.len())
        .map_err(|_| HirFoundationValidationError::Allocation)?;
    for (index, boundary) in native_boundary_types.into_iter().enumerate() {
        boundary_types.push(
            boundary.resolve(identities).map_err(|error| {
                HirFoundationValidationError::NativeBoundaryType { index, error }
            })?,
        );
    }
    native_boundary::validate_shape_coverage(
        &fields,
        &enum_variants,
        &enum_variant_fields,
        &boundary_types,
    )?;

    let mut canonical = CanonicalHirFoundation::empty();
    macro_rules! set {
        ($method:ident, $records:expr) => {
            canonical
                .$method($records)
                .map_err(HirFoundationValidationError::Build)?
        };
    }
    set!(set_sources, validated_sources);
    set!(set_types, types);
    set!(set_generic_types, generic_types);
    set!(set_functions, functions);
    set!(set_generic_functions, generic_functions);
    set!(set_constructors, constructors);
    set!(set_properties, properties);
    set!(set_extension_properties, extension_properties);
    set!(set_object_values, object_values);
    set!(set_type_aliases, type_aliases);
    set!(set_property_accessors, property_accessors);
    set!(set_fields, fields);
    set!(set_enum_variants, enum_variants);
    set!(set_enum_variant_fields, enum_variant_fields);
    set!(set_exact_types, exact_types);
    set!(set_export_bindings, export_bindings);
    set!(set_callable_applications, callable_applications);
    set!(set_generated_callables, generated_callables);
    set!(set_generated_types, generated_types);
    set!(set_dispatch_slots, dispatch_slots);
    set!(set_initialization_units, initialization_units);
    set!(set_source_contexts, source_contexts);
    set!(set_local_bindings, local_bindings);
    set!(set_local_values, local_values);
    set!(set_callback_registrations, callback_registrations);
    set!(set_source_native_contracts, contracts);
    set!(set_odr_groups, odr_groups);
    set!(set_odr_members, odr_members);
    set!(set_definition_origins, origins);
    set!(set_native_boundary_types, boundary_types);

    let rebuilt = encode(&canonical).map_err(HirFoundationValidationError::WireEncode)?;
    if rebuilt != original {
        return Err(HirFoundationValidationError::NonCanonicalFoundation);
    }
    Ok(ValidatedHirFoundation { canonical })
}

#[allow(clippy::too_many_arguments)]
fn validate_declaration_ownership(
    artifact: ConeIdentity,
    types: &[TypeRecord],
    generic_types: &[GenericTypeRecord],
    functions: &[FunctionRecord],
    generic_functions: &[GenericFunctionRecord],
    constructors: &[ConstructorRecord],
    properties: &[PropertyRecord],
    extension_properties: &[ExtensionPropertyRecord],
    object_values: &[ObjectValueRecord],
    type_aliases: &[TypeAliasRecord],
) -> Result<(), HirFoundationValidationError> {
    let core = [
        CoreBuiltinNominal::Unit.identity_record(),
        CoreBuiltinNominal::Any.identity_record(),
    ];
    for builtin in &core {
        if !types.iter().any(|record| record == builtin) {
            return Err(HirFoundationValidationError::MissingCoreBuiltin {
                identity: *builtin.id().as_array(),
            });
        }
    }
    for record in types {
        if record.key().origin() != artifact && !core.iter().any(|builtin| builtin == record) {
            return Err(foreign_declaration(
                HirFoundationTable::Type,
                record.id(),
                record.key().origin(),
                artifact,
            ));
        }
    }
    macro_rules! current_declarations {
        ($(($table:ident, $records:expr)),+ $(,)?) => {
            $(for record in $records {
                if record.key().origin() != artifact {
                    return Err(foreign_declaration(
                        HirFoundationTable::$table,
                        record.id(),
                        record.key().origin(),
                        artifact,
                    ));
                }
            })+
        };
    }
    current_declarations!(
        (GenericType, generic_types),
        (Function, functions),
        (GenericFunction, generic_functions),
        (Constructor, constructors),
        (Property, properties),
        (ExtensionProperty, extension_properties),
        (ObjectValue, object_values),
        (TypeAlias, type_aliases),
    );
    Ok(())
}

fn foreign_declaration<I: PersistentId>(
    table: HirFoundationTable,
    id: I,
    actual: ConeIdentity,
    expected: ConeIdentity,
) -> HirFoundationValidationError {
    HirFoundationValidationError::ForeignDeclaration {
        table,
        identity: *id.as_array(),
        expected,
        actual,
    }
}

fn validate_source_contexts(
    contexts: &[SourceContextRecord],
    sources: &[SourceRecord],
) -> Result<(), HirFoundationValidationError> {
    let known = sources
        .iter()
        .map(|source| source.identity().clone())
        .collect::<BTreeSet<_>>();
    for context in contexts {
        if !known.contains(context.key().source()) {
            return Err(HirFoundationValidationError::UnknownContextSource {
                context: *context.id().as_array(),
                source: context.key().source().clone(),
            });
        }
    }
    Ok(())
}

impl NativeBoundaryResolver<IdentityReferenceError> for ValidatedIdentityGraph {
    fn native_boundary_binder_parameter_counts(
        &mut self,
        declaration: &SourceDeclarationKey,
    ) -> Result<Vec<u32>, IdentityReferenceError> {
        Ok(vec![
            declaration.duplicate_signature().type_parameter_count(),
        ])
    }
}

#[derive(Debug)]
pub enum HirFoundationValidationError {
    CoordinateIdentity(scoop_wire::HashError),
    WireEncode(scoop_wire::cbor::EncodeError),
    Allocation,
    SourceRecord {
        index: usize,
        error: SourceRecordValidationError,
    },
    Identity(IdentityValidationError),
    ForeignDeclaration {
        table: HirFoundationTable,
        identity: [u8; 32],
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    MissingCoreBuiltin {
        identity: [u8; 32],
    },
    UnknownContextSource {
        context: [u8; 32],
        source: scoop_identity::SourceIdentity,
    },
    SourceNativeContract {
        index: usize,
        error: SourceNativeExternalResolutionError<IdentityReferenceError>,
    },
    DefinitionOrigin {
        index: usize,
        error: DefinitionOriginRecordResolutionError<IdentityReferenceError>,
    },
    Origin(DefinitionOriginValidationError),
    NativeBoundaryType {
        index: usize,
        error: NativeBoundaryResolutionError<IdentityReferenceError>,
    },
    NativeBoundaryShapeCoverage(NativeBoundaryShapeCoverageError),
    Build(HirFoundationBuildError),
    NonCanonicalFoundation,
}

impl From<DefinitionOriginValidationError> for HirFoundationValidationError {
    fn from(error: DefinitionOriginValidationError) -> Self {
        Self::Origin(error)
    }
}

impl fmt::Display for HirFoundationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateIdentity(error) => error.fmt(formatter),
            Self::WireEncode(error) => error.fmt(formatter),
            Self::Allocation => formatter.write_str("failed to allocate HIR foundation validation"),
            Self::SourceRecord { index, error } => {
                write!(formatter, "HIR source record {index} is invalid: {error}")
            }
            Self::Identity(error) => error.fmt(formatter),
            Self::ForeignDeclaration {
                table,
                identity,
                expected,
                actual,
            } => write!(
                formatter,
                "{} identity {} belongs to Cone {actual}, not artifact Cone {expected}",
                table.name(),
                HexIdentity(identity),
            ),
            Self::MissingCoreBuiltin { identity } => write!(
                formatter,
                "HIR foundation is missing trusted core nominal {}",
                HexIdentity(identity),
            ),
            Self::UnknownContextSource { context, source } => write!(
                formatter,
                "source context {} refers to absent source {source:?}",
                HexIdentity(context),
            ),
            Self::SourceNativeContract { index, error } => {
                write!(
                    formatter,
                    "HIR source native contract {index} is invalid: {error}"
                )
            }
            Self::DefinitionOrigin { index, error } => {
                write!(
                    formatter,
                    "HIR definition origin {index} is invalid: {error}"
                )
            }
            Self::Origin(error) => error.fmt(formatter),
            Self::NativeBoundaryType { index, error } => {
                write!(
                    formatter,
                    "HIR native-boundary type {index} is invalid: {error}"
                )
            }
            Self::NativeBoundaryShapeCoverage(error) => error.fmt(formatter),
            Self::Build(error) => error.fmt(formatter),
            Self::NonCanonicalFoundation => {
                formatter.write_str("HIR foundation tables are not in canonical structural order")
            }
        }
    }
}

impl From<NativeBoundaryShapeCoverageError> for HirFoundationValidationError {
    fn from(error: NativeBoundaryShapeCoverageError) -> Self {
        Self::NativeBoundaryShapeCoverage(error)
    }
}

impl std::error::Error for HirFoundationValidationError {}

#[cfg(test)]
mod tests;
