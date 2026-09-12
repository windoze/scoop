use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableTemplateOwner, ConeIdentity, DefinitionOriginRecord,
    DefinitionOriginSubject, GeneratedCallableKey, InitializationUnitKey, LocalValueSelector,
    NominalDeclarationOwner, PersistentGeneratedCallableId, PropertyOwner, SourceDeclarationKey,
    SourceIdentity, SourceNativeExternalContractRecord, SourceNativeExternalOwner,
};

use super::super::super::{
    CallbackRegistrationRecord, ConstructorRecord, EnumVariantFieldRecord, EnumVariantRecord,
    ExtensionPropertyRecord, FieldRecord, FunctionRecord, GeneratedCallableRecord,
    GenericFunctionRecord, GenericTypeRecord, InitializationUnitRecord, LocalBindingRecord,
    LocalValueRecord, PropertyAccessorRecord, PropertyRecord, TypeAliasRecord, TypeRecord,
};
use crate::SourceRecord;

#[derive(Clone, Debug)]
enum OriginExpectation {
    Direct {
        cone: ConeIdentity,
        source: Option<SourceIdentity>,
    },
    SameSource(DefinitionOriginSubject),
}

impl OriginExpectation {
    fn declaration(key: &SourceDeclarationKey) -> Self {
        Self::Direct {
            cone: key.origin(),
            source: key.scope().source().cloned(),
        }
    }
}

struct OriginRequirements {
    required: BTreeMap<DefinitionOriginSubject, OriginExpectation>,
    optional: BTreeMap<DefinitionOriginSubject, OriginExpectation>,
    generated: BTreeMap<PersistentGeneratedCallableId, GeneratedCallableKey>,
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
    initialization_units: &[InitializationUnitRecord],
    local_bindings: &[LocalBindingRecord],
    local_values: &[LocalValueRecord],
    callback_registrations: &[CallbackRegistrationRecord],
    native_contracts: &[SourceNativeExternalContractRecord],
    origins: &[DefinitionOriginRecord],
) -> Result<(), DefinitionOriginValidationError> {
    let mut requirements = OriginRequirements::new(generated_callables);

    for record in types {
        if record.key().origin() != ConeIdentity::CORE {
            requirements.require(
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
        if let Some(owner) = record.key().source_owner() {
            requirements.require(
                DefinitionOriginSubject::Field(record.id()),
                OriginExpectation::SameSource(nominal_subject(owner)),
            )?;
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
    let source_variants = enum_variants
        .iter()
        .filter(|record| record.key().source_owner().is_some())
        .map(|record| record.id())
        .collect::<BTreeSet<_>>();
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
    for record in initialization_units {
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
                source: Some(record.key().source().clone()),
            },
        )?;
    }
    for record in local_values {
        if matches!(
            record.key().selector(),
            LocalValueSelector::This
                | LocalValueSelector::Parameter { .. }
                | LocalValueSelector::LocalDeclaration { .. }
                | LocalValueSelector::BoundReceiver { .. }
        ) {
            let anchor = requirements
                .materialization_subject(record.key().owner())
                .ok_or(DefinitionOriginValidationError::MissingSourceAnchor {
                    subject: DefinitionOriginSubject::LocalValue(record.id()),
                })?;
            requirements.require(
                DefinitionOriginSubject::LocalValue(record.id()),
                OriginExpectation::SameSource(anchor),
            )?;
        }
    }
    for record in callback_registrations {
        let subject = DefinitionOriginSubject::CallbackRegistration(record.id());
        let anchor = requirements
            .callable_subject(record.key().parent().template(), &mut BTreeSet::new())
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
        if let Some(anchor) = requirements.generated_subject(record.id(), &mut BTreeSet::new()) {
            requirements.allow(subject, OriginExpectation::SameSource(anchor))?;
        }
    }

    validate_records(artifact, sources, requirements, origins)
}

impl OriginRequirements {
    fn new(generated: &[GeneratedCallableRecord]) -> Self {
        Self {
            required: BTreeMap::new(),
            optional: BTreeMap::new(),
            generated: generated
                .iter()
                .map(|record| (record.id(), record.key().clone()))
                .collect(),
        }
    }

    fn require(
        &mut self,
        subject: DefinitionOriginSubject,
        expectation: OriginExpectation,
    ) -> Result<(), DefinitionOriginValidationError> {
        if self.required.insert(subject, expectation).is_some() {
            Err(DefinitionOriginValidationError::DuplicateRequirement { subject })
        } else {
            Ok(())
        }
    }

    fn allow(
        &mut self,
        subject: DefinitionOriginSubject,
        expectation: OriginExpectation,
    ) -> Result<(), DefinitionOriginValidationError> {
        if self.optional.insert(subject, expectation).is_some() {
            Err(DefinitionOriginValidationError::DuplicateRequirement { subject })
        } else {
            Ok(())
        }
    }

    fn materialization_subject(
        &self,
        materialization: CallableMaterialization,
    ) -> Option<DefinitionOriginSubject> {
        self.callable_subject(materialization.template(), &mut BTreeSet::new())
    }

    fn callable_subject(
        &self,
        owner: CallableTemplateOwner,
        visiting: &mut BTreeSet<PersistentGeneratedCallableId>,
    ) -> Option<DefinitionOriginSubject> {
        match owner {
            CallableTemplateOwner::Function(id) => Some(DefinitionOriginSubject::Function(id)),
            CallableTemplateOwner::GenericFunction(id) => {
                Some(DefinitionOriginSubject::GenericFunction(id))
            }
            CallableTemplateOwner::Constructor(id) => {
                Some(DefinitionOriginSubject::Constructor(id))
            }
            CallableTemplateOwner::Accessor(id) => {
                Some(DefinitionOriginSubject::PropertyAccessor(id))
            }
            CallableTemplateOwner::Generated(id) => self.generated_subject(id, visiting),
            CallableTemplateOwner::VariantConstructor(id) => {
                Some(DefinitionOriginSubject::EnumVariant(id))
            }
        }
    }

    fn generated_subject(
        &self,
        id: PersistentGeneratedCallableId,
        visiting: &mut BTreeSet<PersistentGeneratedCallableId>,
    ) -> Option<DefinitionOriginSubject> {
        if !visiting.insert(id) {
            return None;
        }
        let result = match self.generated.get(&id)? {
            GeneratedCallableKey::Lexical { parent, .. }
            | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                self.callable_subject(parent.template(), visiting)
            }
            GeneratedCallableKey::Initialization { unit, .. } => {
                Some(DefinitionOriginSubject::InitializationUnit(*unit))
            }
            GeneratedCallableKey::StaticNoGcCallbackStorageBridge { source, .. }
            | GeneratedCallableKey::CoroutineDriver {
                source_callable: source,
            }
            | GeneratedCallableKey::CoroutineAdapter {
                source_callable: source,
                ..
            }
            | GeneratedCallableKey::DispatchAdjust { target: source, .. } => {
                self.callable_subject(source.template(), visiting)
            }
            GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                Some(DefinitionOriginSubject::Constructor(*constructor))
            }
            GeneratedCallableKey::DerivedEquality { .. }
            | GeneratedCallableKey::FunctionAdapter { .. }
            | GeneratedCallableKey::DynamicFunctionAdapter { .. }
            | GeneratedCallableKey::ForeignCallbackManagedAdapter { .. }
            | GeneratedCallableKey::ContinuationShell { .. }
            | GeneratedCallableKey::CoroutineStart { .. }
            | GeneratedCallableKey::FunctionBridge { .. }
            | GeneratedCallableKey::BoxingAdjust { .. } => None,
        };
        visiting.remove(&id);
        result
    }
}

fn validate_records(
    artifact: ConeIdentity,
    sources: &[SourceRecord],
    requirements: OriginRequirements,
    origins: &[DefinitionOriginRecord],
) -> Result<(), DefinitionOriginValidationError> {
    let source_records = sources
        .iter()
        .map(|source| (source.identity().clone(), source))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::new();
    for record in origins {
        if actual.insert(record.subject(), record).is_some() {
            return Err(DefinitionOriginValidationError::DuplicateSubject {
                subject: record.subject(),
            });
        }
        if !requirements.required.contains_key(&record.subject())
            && !requirements.optional.contains_key(&record.subject())
        {
            return Err(DefinitionOriginValidationError::UnexpectedSubject {
                subject: record.subject(),
            });
        }
    }
    if let Some(subject) = requirements
        .required
        .keys()
        .find(|subject| !actual.contains_key(subject))
    {
        return Err(DefinitionOriginValidationError::MissingSubject { subject: *subject });
    }

    for record in origins {
        let subject = record.subject();
        let expectation = requirements
            .required
            .get(&subject)
            .or_else(|| requirements.optional.get(&subject))
            .expect("origin membership was checked above");
        let source = record.origin().source();
        match expectation {
            OriginExpectation::Direct {
                cone,
                source: exact,
            } => {
                if source.cone() != *cone || source.cone() != artifact {
                    return Err(DefinitionOriginValidationError::SourceConeMismatch {
                        subject,
                        expected: *cone,
                        actual: source.cone(),
                    });
                }
                if let Some(expected) = exact
                    && source != expected
                {
                    return Err(DefinitionOriginValidationError::SourceMismatch {
                        subject,
                        expected: Box::new(expected.clone()),
                        actual: Box::new(source.clone()),
                    });
                }
            }
            OriginExpectation::SameSource(anchor) => {
                let Some(anchor_record) = actual.get(anchor) else {
                    return Err(DefinitionOriginValidationError::MissingSourceAnchor { subject });
                };
                let expected = anchor_record.origin().source();
                if source != expected {
                    return Err(DefinitionOriginValidationError::SourceMismatch {
                        subject,
                        expected: Box::new(expected.clone()),
                        actual: Box::new(source.clone()),
                    });
                }
            }
        }
        let Some(source_record) = source_records.get(source) else {
            return Err(DefinitionOriginValidationError::UnknownSource {
                subject,
                source: Box::new(source.clone()),
            });
        };
        let span = record.origin().span();
        source_record
            .require_points([span.start_byte(), span.end_byte()])
            .map_err(|error| DefinitionOriginValidationError::MissingPoint {
                subject,
                source: Box::new(source.clone()),
                byte_offset: error.byte_offset,
            })?;
    }
    Ok(())
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionOriginValidationError {
    DuplicateRequirement {
        subject: DefinitionOriginSubject,
    },
    DuplicateSubject {
        subject: DefinitionOriginSubject,
    },
    MissingSubject {
        subject: DefinitionOriginSubject,
    },
    UnexpectedSubject {
        subject: DefinitionOriginSubject,
    },
    MissingSourceAnchor {
        subject: DefinitionOriginSubject,
    },
    SourceConeMismatch {
        subject: DefinitionOriginSubject,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    SourceMismatch {
        subject: DefinitionOriginSubject,
        expected: Box<SourceIdentity>,
        actual: Box<SourceIdentity>,
    },
    UnknownSource {
        subject: DefinitionOriginSubject,
        source: Box<SourceIdentity>,
    },
    MissingPoint {
        subject: DefinitionOriginSubject,
        source: Box<SourceIdentity>,
        byte_offset: u64,
    },
}

impl fmt::Display for DefinitionOriginValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRequirement { subject } => {
                write!(
                    formatter,
                    "HIR identity graph repeats origin subject {subject:?}"
                )
            }
            Self::DuplicateSubject { subject } => {
                write!(
                    formatter,
                    "HIR definition origins repeat subject {subject:?}"
                )
            }
            Self::MissingSubject { subject } => {
                write!(
                    formatter,
                    "HIR definition origin is missing for {subject:?}"
                )
            }
            Self::UnexpectedSubject { subject } => {
                write!(
                    formatter,
                    "HIR definition origin is not allowed for {subject:?}"
                )
            }
            Self::MissingSourceAnchor { subject } => write!(
                formatter,
                "HIR definition origin for {subject:?} has no unique source-backed owner"
            ),
            Self::SourceConeMismatch {
                subject,
                expected,
                actual,
            } => write!(
                formatter,
                "HIR definition origin for {subject:?} belongs to Cone {actual}, expected {expected}"
            ),
            Self::SourceMismatch {
                subject,
                expected,
                actual,
            } => write!(
                formatter,
                "HIR definition origin for {subject:?} uses source {actual:?}, expected {expected:?}"
            ),
            Self::UnknownSource { subject, source } => write!(
                formatter,
                "HIR definition origin for {subject:?} refers to absent source {source:?}"
            ),
            Self::MissingPoint {
                subject,
                source,
                byte_offset,
            } => write!(
                formatter,
                "HIR definition origin for {subject:?} refers to missing point {byte_offset} in {source:?}"
            ),
        }
    }
}

impl std::error::Error for DefinitionOriginValidationError {}
