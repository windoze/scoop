use std::collections::{HashMap, HashSet};
use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableTemplateOwner, ConeIdentity, CoreBuiltinNominal,
    DefinitionOriginRecord, DefinitionOriginSubject, GeneratedCallableKey, InitializationUnitKey,
    LocalValueSelector, NominalDeclarationOwner, PersistentGeneratedCallableId, PropertyOwner,
    SourceDeclarationKey, SourceIdentity, SourceNativeExternalContractRecord,
    SourceNativeExternalOwner,
};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::super::super::{
    CallbackRegistrationRecord, ConstructorRecord, EnumVariantFieldRecord, EnumVariantRecord,
    ExtensionPropertyRecord, FieldRecord, FunctionRecord, GeneratedCallableRecord,
    GenericFunctionRecord, GenericTypeRecord, InitializationUnitRecord, LocalBindingRecord,
    LocalValueRecord, PropertyAccessorRecord, PropertyRecord, TypeAliasRecord, TypeRecord,
};
use super::HirFoundationValidationError;
use crate::SourceRecord;

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

mod fields;

fn type_requires_definition_origin(record: &TypeRecord) -> bool {
    let id = record.id();
    id != CoreBuiltinNominal::Unit.identity_record().id()
        && id != CoreBuiltinNominal::Any.identity_record().id()
}

struct OriginRequirements<'a> {
    required: HashMap<DefinitionOriginSubject, OriginExpectation<'a>>,
    optional: HashMap<DefinitionOriginSubject, OriginExpectation<'a>>,
    generated: HashMap<PersistentGeneratedCallableId, &'a GeneratedCallableKey>,
    visiting: HashSet<PersistentGeneratedCallableId>,
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
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationValidationError> {
    let path = WirePath::root().field(29);
    let mut source_variants = HashSet::new();
    let source_variant_count = enum_variants
        .iter()
        .filter(|record| record.key().source_owner().is_some())
        .count();
    meter
        .try_reserve_set_slots(&mut source_variants, source_variant_count, &path)
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
    let mut requirements =
        OriginRequirements::new(generated_callables, required_count, meter, &path)?;

    for record in types {
        if type_requires_definition_origin(record) {
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
                source: Some(record.key().source()),
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
                .materialization_subject(record.key().owner(), meter, &path)?
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
            .callable_subject(record.key().parent().template(), meter, &path)?
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
        if let Some(anchor) = requirements.generated_subject(record.id(), meter, &path)? {
            requirements.allow(subject, OriginExpectation::SameSource(anchor))?;
        }
    }

    validate_records(artifact, sources, requirements, origins, meter)
}

impl<'a> OriginRequirements<'a> {
    fn new(
        generated_records: &'a [GeneratedCallableRecord],
        required_count: usize,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, HirFoundationValidationError> {
        let mut required = HashMap::new();
        meter
            .try_reserve_map_slots(&mut required, required_count, path)
            .map_err(HirFoundationValidationError::Resource)?;
        let mut optional = HashMap::new();
        meter
            .try_reserve_map_slots(&mut optional, generated_records.len(), path)
            .map_err(HirFoundationValidationError::Resource)?;
        let mut generated = HashMap::new();
        meter
            .try_reserve_map_slots(&mut generated, generated_records.len(), path)
            .map_err(HirFoundationValidationError::Resource)?;
        generated.extend(
            generated_records
                .iter()
                .map(|record| (record.id(), record.key())),
        );
        let mut visiting = HashSet::new();
        meter
            .try_reserve_set_slots(&mut visiting, generated_records.len(), path)
            .map_err(HirFoundationValidationError::Resource)?;
        Ok(Self {
            required,
            optional,
            generated,
            visiting,
        })
    }

    fn require(
        &mut self,
        subject: DefinitionOriginSubject,
        expectation: OriginExpectation<'a>,
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
        expectation: OriginExpectation<'a>,
    ) -> Result<(), DefinitionOriginValidationError> {
        if self.optional.insert(subject, expectation).is_some() {
            Err(DefinitionOriginValidationError::DuplicateRequirement { subject })
        } else {
            Ok(())
        }
    }

    fn materialization_subject(
        &mut self,
        materialization: CallableMaterialization,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        self.callable_subject(materialization.template(), meter, path)
    }

    fn callable_subject(
        &mut self,
        owner: CallableTemplateOwner,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        self.visiting.clear();
        self.resolve_callable_subject(owner, meter, path)
    }

    fn generated_subject(
        &mut self,
        id: PersistentGeneratedCallableId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        self.visiting.clear();
        self.resolve_callable_subject(CallableTemplateOwner::Generated(id), meter, path)
    }

    fn resolve_callable_subject(
        &mut self,
        mut owner: CallableTemplateOwner,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        let mut depth = 1_u64;
        loop {
            meter
                .check_semantic_depth(depth, path)
                .map_err(HirFoundationValidationError::Resource)?;
            match owner {
                CallableTemplateOwner::Function(id) => {
                    return Ok(Some(DefinitionOriginSubject::Function(id)));
                }
                CallableTemplateOwner::GenericFunction(id) => {
                    return Ok(Some(DefinitionOriginSubject::GenericFunction(id)));
                }
                CallableTemplateOwner::Constructor(id) => {
                    return Ok(Some(DefinitionOriginSubject::Constructor(id)));
                }
                CallableTemplateOwner::Accessor(id) => {
                    return Ok(Some(DefinitionOriginSubject::PropertyAccessor(id)));
                }
                CallableTemplateOwner::VariantConstructor(id) => {
                    return Ok(Some(DefinitionOriginSubject::EnumVariant(id)));
                }
                CallableTemplateOwner::Generated(id) => {
                    if !self.visiting.insert(id) {
                        return Ok(None);
                    }
                    meter
                        .charge_edges(1, path)
                        .map_err(HirFoundationValidationError::Resource)?;
                    let Some(key) = self.generated.get(&id) else {
                        return Ok(None);
                    };
                    match key {
                        GeneratedCallableKey::Lexical { parent, .. }
                        | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                            owner = parent.template();
                        }
                        GeneratedCallableKey::Initialization { unit, .. } => {
                            return Ok(Some(DefinitionOriginSubject::InitializationUnit(*unit)));
                        }
                        GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                            source, ..
                        }
                        | GeneratedCallableKey::CoroutineDriver {
                            source_callable: source,
                        }
                        | GeneratedCallableKey::CoroutineAdapter {
                            source_callable: source,
                            ..
                        }
                        | GeneratedCallableKey::DispatchAdjust { target: source, .. } => {
                            owner = source.template();
                        }
                        GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                            return Ok(Some(DefinitionOriginSubject::Constructor(*constructor)));
                        }
                        GeneratedCallableKey::DerivedEquality { .. }
                        | GeneratedCallableKey::FunctionAdapter { .. }
                        | GeneratedCallableKey::DynamicFunctionAdapter { .. }
                        | GeneratedCallableKey::ForeignCallbackManagedAdapter { .. }
                        | GeneratedCallableKey::ContinuationShell { .. }
                        | GeneratedCallableKey::CoroutineStart { .. }
                        | GeneratedCallableKey::FunctionBridge { .. }
                        | GeneratedCallableKey::BoxingAdjust { .. } => return Ok(None),
                    }
                    depth = depth.checked_add(1).ok_or_else(|| {
                        HirFoundationValidationError::Resource(WireError::new(
                            WireErrorKind::IntegerOutOfRange,
                            path.clone(),
                            None,
                        ))
                    })?;
                }
            }
        }
    }
}

fn validate_records(
    artifact: ConeIdentity,
    sources: &[SourceRecord],
    requirements: OriginRequirements<'_>,
    origins: &[DefinitionOriginRecord],
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationValidationError> {
    let source_path = WirePath::root().field(1);
    let mut source_records = HashMap::new();
    meter
        .try_reserve_map_slots(&mut source_records, sources.len(), &source_path)
        .map_err(HirFoundationValidationError::Resource)?;
    source_records.extend(sources.iter().map(|source| (source.identity(), source)));
    let origin_path = WirePath::root().field(29);
    let mut actual = HashMap::new();
    meter
        .try_reserve_map_slots(&mut actual, origins.len(), &origin_path)
        .map_err(HirFoundationValidationError::Resource)?;
    for record in origins {
        if actual.insert(record.subject(), record).is_some() {
            return Err(DefinitionOriginValidationError::DuplicateSubject {
                subject: record.subject(),
            }
            .into());
        }
        if !requirements.required.contains_key(&record.subject())
            && !requirements.optional.contains_key(&record.subject())
        {
            return Err(DefinitionOriginValidationError::UnexpectedSubject {
                subject: record.subject(),
            }
            .into());
        }
    }
    if let Some(subject) = requirements
        .required
        .keys()
        .filter(|subject| !actual.contains_key(subject))
        .min()
    {
        return Err(DefinitionOriginValidationError::MissingSubject { subject: *subject }.into());
    }

    for record in origins {
        let subject = record.subject();
        let Some(expectation) = requirements
            .required
            .get(&subject)
            .or_else(|| requirements.optional.get(&subject))
        else {
            return Err(DefinitionOriginValidationError::UnexpectedSubject { subject }.into());
        };
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
                    }
                    .into());
                }
                if let Some(expected) = exact
                    && source != *expected
                {
                    let expected = copy_source_for_error(expected, meter, &origin_path)?;
                    let actual = copy_source_for_error(source, meter, &origin_path)?;
                    return Err(DefinitionOriginValidationError::SourceMismatch {
                        subject,
                        expected,
                        actual,
                    }
                    .into());
                }
            }
            OriginExpectation::SameSource(anchor) => {
                let Some(anchor_record) = actual.get(anchor) else {
                    return Err(
                        DefinitionOriginValidationError::MissingSourceAnchor { subject }.into(),
                    );
                };
                let expected = anchor_record.origin().source();
                if source != expected {
                    let expected = copy_source_for_error(expected, meter, &origin_path)?;
                    let actual = copy_source_for_error(source, meter, &origin_path)?;
                    return Err(DefinitionOriginValidationError::SourceMismatch {
                        subject,
                        expected,
                        actual,
                    }
                    .into());
                }
            }
        }
        let Some(source_record) = source_records.get(source) else {
            return Err(DefinitionOriginValidationError::UnknownSource {
                subject,
                source: copy_source_for_error(source, meter, &origin_path)?,
            }
            .into());
        };
        let span = record.origin().span();
        if let Err(error) = source_record.require_points([span.start_byte(), span.end_byte()]) {
            return Err(DefinitionOriginValidationError::MissingPoint {
                subject,
                source: copy_source_for_error(source, meter, &origin_path)?,
                byte_offset: error.byte_offset,
            }
            .into());
        }
    }
    Ok(())
}

fn copy_source_for_error(
    source: &SourceIdentity,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Box<SourceIdentity>, HirFoundationValidationError> {
    let owned_bytes = u64::try_from(source.logical_path().as_str().len()).map_err(|_| {
        HirFoundationValidationError::Resource(WireError::new(
            WireErrorKind::IntegerOutOfRange,
            path.clone(),
            None,
        ))
    })?;
    meter
        .charge_owned_bytes(owned_bytes, path)
        .map_err(HirFoundationValidationError::Resource)?;
    Ok(Box::new(source.clone()))
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

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain,
        LexicalCallableParent, LexicalCallableRole, PackagePath, PersistentFunctionId,
        SourceDeclarationSite, StructuralDefinitionPath, StructuralDefinitionSiteRole,
        StructuralPathSegment,
    };
    use scoop_wire::{DecodeLimits, ResourceKind};

    use super::*;

    #[test]
    fn generated_origin_walk_obeys_the_semantic_depth_limit() {
        let function_key = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("origin_depth_root").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
        let parent_key = GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(function),
            role: LexicalCallableRole::LambdaBody,
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
                [],
            ),
        };
        let parent = CborIdentityRecord::from_key(parent_key).unwrap();
        let child = CborIdentityRecord::from_key(GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::from_generated_key(parent.key()).unwrap(),
            role: LexicalCallableRole::AnonymousFunctionBody,
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 1),
                [],
            ),
        })
        .unwrap();
        let child_id = child.id();
        let records = vec![parent, child];
        let path = WirePath::root().field(29);
        let mut meter = BudgetMeter::new(DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        });
        let mut requirements = OriginRequirements::new(&records, 0, &mut meter, &path).unwrap();

        let error = requirements
            .generated_subject(child_id, &mut meter, &path)
            .unwrap_err();
        assert!(matches!(
            error,
            HirFoundationValidationError::Resource(ref error)
                if error.kind() == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::SemanticRecursion,
                    limit: 1,
                    observed: 2,
                }
        ));
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
