use std::collections::HashSet;

use scoop_identity::{
    ConeIdentity, CoreBuiltinNominal, DefinitionOriginRecord, DefinitionOriginSubject,
    InitializationUnitKey, LocalValueSelector, NominalDeclarationOwner, PropertyOwner,
    SourceDeclarationKey, SourceIdentity, SourceNativeExternalContractRecord,
    SourceNativeExternalOwner,
};
use scoop_wire::{WireError, WireErrorKind, WirePath};

use super::super::super::{
    CallbackRegistrationRecord, ConstructorRecord, EnumVariantFieldRecord, EnumVariantRecord,
    ExtensionPropertyRecord, FieldRecord, FunctionRecord, GeneratedCallableRecord,
    GenericFunctionRecord, GenericTypeRecord, InitializationUnitRecord, LocalBindingRecord,
    LocalValueRecord, PropertyAccessorRecord, PropertyRecord, TypeAliasRecord, TypeRecord,
};
use super::HirFoundationValidationError;
use crate::{CanonicalHirFoundation, SourceRecord};

#[derive(Clone, Copy, Debug)]
enum OriginExpectation<'a> {
    Direct {
        cone: ConeIdentity,
        source: Option<&'a SourceIdentity>,
    },
    SameSource(DefinitionOriginSubject),
}

impl<'a> OriginExpectation<'a> {
    fn declaration(key: &'a SourceDeclarationKey) -> Self {
        Self::Direct {
            cone: key.origin(),
            source: key.scope().source(),
        }
    }
}

mod errors;
mod fields;
mod records;
mod requirements;

pub use errors::DefinitionOriginValidationError;
use records::validate_records;
use requirements::OriginRequirements;

fn type_requires_definition_origin(record: &TypeRecord) -> bool {
    let id = record.id();
    id != CoreBuiltinNominal::Unit.identity_record().id()
        && id != CoreBuiltinNominal::Any.identity_record().id()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    artifact: ConeIdentity,
    sources: &[SourceRecord],
    types: &[TypeRecord],
    generic_types: &[GenericTypeRecord],
    functions: &[FunctionRecord],
    generic_functions: &[GenericFunctionRecord],
    constructors: &[ConstructorRecord],
    properties: &[PropertyRecord],
    extension_properties: &[ExtensionPropertyRecord],
    type_aliases: &[TypeAliasRecord],
    property_accessors: &[PropertyAccessorRecord],
    fields: &[FieldRecord],
    enum_variants: &[EnumVariantRecord],
    enum_variant_fields: &[EnumVariantFieldRecord],
    generated_callables: &[GeneratedCallableRecord],
    exact_types: &[crate::foundation::ExactTypeRecord],
    initialization_units: &[InitializationUnitRecord],
    local_bindings: &[LocalBindingRecord],
    local_values: &[LocalValueRecord],
    callback_registrations: &[CallbackRegistrationRecord],
    native_contracts: &[SourceNativeExternalContractRecord],
    origins: &[DefinitionOriginRecord],
    dependencies: &[&CanonicalHirFoundation],
) -> Result<(), HirFoundationValidationError> {
    let path = WirePath::root().field(29);
    let mut source_variants = HashSet::new();
    let source_variant_count = enum_variants
        .iter()
        .filter(|record| record.key().source_owner().is_some())
        .count();
    scoop_wire::allocation::try_reserve_set(&mut source_variants, source_variant_count, &path)
        .map_err(HirFoundationValidationError::Resource)?;
    source_variants.extend(
        enum_variants
            .iter()
            .filter(|record| record.key().source_owner().is_some())
            .map(|record| record.id()),
    );

    let required_count = checked_sum(
        [
            types
                .iter()
                .filter(|record| type_requires_definition_origin(record))
                .count(),
            generic_types.len(),
            functions.len(),
            generic_functions.len(),
            constructors.len(),
            properties.len(),
            extension_properties.len(),
            type_aliases.len(),
            property_accessors.len(),
            fields
                .iter()
                .filter(|record| fields::expectation(record.key()).is_some())
                .count(),
            source_variant_count,
            enum_variant_fields
                .iter()
                .filter(|record| source_variants.contains(&record.key().variant()))
                .count(),
            initialization_units.len(),
            local_bindings.len(),
            local_values
                .iter()
                .filter(|record| {
                    matches!(
                        record.key().selector(),
                        LocalValueSelector::This
                            | LocalValueSelector::Parameter { .. }
                            | LocalValueSelector::LocalDeclaration { .. }
                            | LocalValueSelector::BoundReceiver { .. }
                    )
                })
                .count(),
            callback_registrations.len(),
            native_contracts.len(),
        ],
        &path,
    )?;
    let mut requirements = OriginRequirements::new(
        generated_callables,
        exact_types,
        dependencies,
        required_count,
        &path,
    )?;

    for record in types {
        if type_requires_definition_origin(record) {
            requirements.require(
                DefinitionOriginSubject::Type(record.id()),
                OriginExpectation::declaration(record.key()),
            )?;
        } else if record.id() == CoreBuiltinNominal::Unit.identity_record().id() {
            // The language key also exists without a local declaration. When
            // core publishes Unit's source declaration, retain its normal source location.
            requirements.allow(
                DefinitionOriginSubject::Type(record.id()),
                OriginExpectation::declaration(record.key()),
            )?;
        }
    }
    macro_rules! declarations {
        ($(($records:expr, $subject:ident)),+ $(,)?) => {
            $(for record in $records {
                requirements.require(
                    DefinitionOriginSubject::$subject(record.id()),
                    OriginExpectation::declaration(record.key()),
                )?;
            })+
        };
    }
    declarations!(
        (generic_types, GenericType),
        (functions, Function),
        (generic_functions, GenericFunction),
        (constructors, Constructor),
        (properties, Property),
        (extension_properties, ExtensionProperty),
        (type_aliases, TypeAlias),
    );

    for record in property_accessors {
        requirements.require(
            DefinitionOriginSubject::PropertyAccessor(record.id()),
            OriginExpectation::SameSource(property_subject(record.key().owner())),
        )?;
    }
    for record in fields {
        if let Some(expectation) = fields::expectation(record.key()) {
            requirements.require(DefinitionOriginSubject::Field(record.id()), expectation)?;
        }
    }
    for record in enum_variants {
        if let Some(owner) = record.key().source_owner() {
            requirements.require(
                DefinitionOriginSubject::EnumVariant(record.id()),
                OriginExpectation::SameSource(nominal_subject(owner)),
            )?;
        }
    }
    for record in enum_variant_fields {
        if source_variants.contains(&record.key().variant()) {
            requirements.require(
                DefinitionOriginSubject::EnumVariantField(record.id()),
                OriginExpectation::SameSource(DefinitionOriginSubject::EnumVariant(
                    record.key().variant(),
                )),
            )?;
        }
    }
    // Applications retain the source origin through their property declaration.
    for record in initialization_units
        .iter()
        .filter(|record| record.key().specialization_key().is_none())
    {
        let subject = DefinitionOriginSubject::InitializationUnit(record.id());
        requirements.require(
            subject,
            OriginExpectation::SameSource(initialization_subject(record.key())),
        )?;
    }
    for record in local_bindings {
        requirements.require(
            DefinitionOriginSubject::LocalBinding(record.id()),
            OriginExpectation::Direct {
                cone: artifact,
                source: Some(record.key().source()),
            },
        )?;
    }
    for record in local_values {
        requirements.local_value(record, &path)?;
    }
    for record in callback_registrations {
        let subject = DefinitionOriginSubject::CallbackRegistration(record.id());
        let anchor = requirements
            .callable_subject(record.key().parent().template(), &path)?
            .ok_or(DefinitionOriginValidationError::MissingSourceAnchor { subject })?;
        requirements.require(subject, OriginExpectation::SameSource(anchor))?;
    }
    for record in native_contracts {
        requirements.require(
            DefinitionOriginSubject::SourceNativeContract(record.id()),
            OriginExpectation::SameSource(native_owner_subject(record.key().owner())),
        )?;
    }
    for record in generated_callables {
        let subject = DefinitionOriginSubject::GeneratedCallable(record.id());
        if let Some(anchor) = requirements.generated_subject(record.id(), &path)? {
            requirements.allow(subject, OriginExpectation::SameSource(anchor))?;
        }
    }

    validate_records(artifact, sources, requirements, origins, dependencies)
}

fn checked_sum(
    counts: impl IntoIterator<Item = usize>,
    path: &WirePath,
) -> Result<usize, HirFoundationValidationError> {
    let count = counts.into_iter().try_fold(0_u64, |total, count| {
        let count = u64::try_from(count).map_err(|_| {
            HirFoundationValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        total.checked_add(count).ok_or_else(|| {
            HirFoundationValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })
    })?;
    usize::try_from(count).map_err(|_| {
        HirFoundationValidationError::Resource(WireError::new(
            WireErrorKind::IntegerOutOfRange,
            path.clone(),
            None,
        ))
    })
}

fn property_subject(owner: PropertyOwner) -> DefinitionOriginSubject {
    match owner {
        PropertyOwner::Property(id) => DefinitionOriginSubject::Property(id),
        PropertyOwner::ExtensionProperty(id) => DefinitionOriginSubject::ExtensionProperty(id),
    }
}

fn nominal_subject(owner: NominalDeclarationOwner) -> DefinitionOriginSubject {
    match owner {
        NominalDeclarationOwner::Concrete(id) => DefinitionOriginSubject::Type(id),
        NominalDeclarationOwner::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
    }
}

fn initialization_subject(key: &InitializationUnitKey) -> DefinitionOriginSubject {
    match key {
        InitializationUnitKey::TopLevelProperty(id) => DefinitionOriginSubject::Property(*id),
        InitializationUnitKey::ExtensionProperty(id)
        | InitializationUnitKey::GenericDelegatedExtensionApplication { property: id, .. } => {
            DefinitionOriginSubject::ExtensionProperty(*id)
        }
        InitializationUnitKey::GenericCompanionTemplate(id)
        | InitializationUnitKey::GenericCompanionApplication { companion: id, .. } => {
            DefinitionOriginSubject::GenericType(*id)
        }
        InitializationUnitKey::Object(id) | InitializationUnitKey::Companion(id) => {
            DefinitionOriginSubject::Type(*id)
        }
    }
}

fn native_owner_subject(owner: SourceNativeExternalOwner) -> DefinitionOriginSubject {
    match owner {
        SourceNativeExternalOwner::Function(id) => DefinitionOriginSubject::Function(id),
        SourceNativeExternalOwner::Property(id) => DefinitionOriginSubject::Property(id),
    }
}
