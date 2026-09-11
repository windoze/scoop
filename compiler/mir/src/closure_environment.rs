//! Persistent identity bundle for source closure environments.

use std::fmt;

use scoop_identity::{
    CallableMaterialization, CborIdentityRecord, ClosureEnvironmentRole, FieldIdentityError,
    FieldIdentityKey, GeneratedNominalIdentityError, GeneratedNominalKey, LocalValueKey,
    PersistentFieldId, PersistentLocalValueId, PersistentTypeId,
};

use crate::{ClosureClass, ClosureClassId};

type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type LocalValueRecord = CborIdentityRecord<PersistentLocalValueId, LocalValueKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;

/// Semantic source of one physical closure-environment field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClosureFieldSource {
    Capture { declaration_index: u32 },
    CallableReferenceReceiver,
}

/// Persistent identity of one physical closure-environment field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureFieldIdentity {
    source: ClosureFieldSource,
    value: LocalValueRecord,
    field: FieldRecord,
}

impl ClosureFieldIdentity {
    pub const fn source(&self) -> ClosureFieldSource {
        self.source
    }

    pub const fn value_record(&self) -> &LocalValueRecord {
        &self.value
    }

    pub const fn field_record(&self) -> &FieldRecord {
        &self.field
    }
}

/// Complete persistent identity projection for one source closure class.
///
/// `fields` is in physical layout order, which is the raw persistent local-
/// value id order required by the identity schema. `source` independently
/// retains the language evaluation order selector used by MIR lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureEnvironmentIdentity {
    generated_type: GeneratedTypeRecord,
    fields: Vec<ClosureFieldIdentity>,
}

impl ClosureEnvironmentIdentity {
    pub fn for_lambda(
        callable: CallableMaterialization,
        inputs: Vec<(ClosureFieldSource, LocalValueRecord)>,
    ) -> Result<Self, ClosureEnvironmentIdentityError> {
        Self::new(callable, ClosureEnvironmentRole::Lambda, inputs)
    }

    pub fn for_anonymous_function(
        callable: CallableMaterialization,
        inputs: Vec<(ClosureFieldSource, LocalValueRecord)>,
    ) -> Result<Self, ClosureEnvironmentIdentityError> {
        Self::new(callable, ClosureEnvironmentRole::AnonymousFunction, inputs)
    }

    pub fn for_callable_reference(
        callable: CallableMaterialization,
        inputs: Vec<(ClosureFieldSource, LocalValueRecord)>,
    ) -> Result<Self, ClosureEnvironmentIdentityError> {
        Self::new(callable, ClosureEnvironmentRole::CallableReference, inputs)
    }

    pub fn new(
        callable: CallableMaterialization,
        role: ClosureEnvironmentRole,
        inputs: Vec<(ClosureFieldSource, LocalValueRecord)>,
    ) -> Result<Self, ClosureEnvironmentIdentityError> {
        let mut sources = inputs.iter().map(|(source, _)| *source).collect::<Vec<_>>();
        sources.sort_unstable();
        if sources.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ClosureEnvironmentIdentityError::DuplicateSource);
        }

        let receiver_count = sources
            .iter()
            .filter(|source| matches!(source, ClosureFieldSource::CallableReferenceReceiver))
            .count();
        if receiver_count != 0 && role != ClosureEnvironmentRole::CallableReference {
            return Err(ClosureEnvironmentIdentityError::ReceiverRoleMismatch);
        }

        let capture_indices = sources
            .iter()
            .filter_map(|source| match source {
                ClosureFieldSource::Capture { declaration_index } => Some(*declaration_index),
                ClosureFieldSource::CallableReferenceReceiver => None,
            })
            .collect::<Vec<_>>();
        if capture_indices
            .iter()
            .enumerate()
            .any(|(expected, actual)| u32::try_from(expected).ok().as_ref() != Some(actual))
        {
            return Err(ClosureEnvironmentIdentityError::NonContiguousCaptures);
        }

        if inputs
            .iter()
            .any(|(_, value)| value.key().owner().context() != callable.context())
        {
            return Err(ClosureEnvironmentIdentityError::MaterializationContextMismatch);
        }

        let generated_type =
            CborIdentityRecord::from_key(GeneratedNominalKey::ClosureEnvironment {
                callable,
                role,
            })
            .map_err(ClosureEnvironmentIdentityError::GeneratedType)?;
        let mut fields = inputs
            .into_iter()
            .map(|(source, value)| {
                let key = match source {
                    ClosureFieldSource::Capture { .. } => {
                        FieldIdentityKey::closure_capture(generated_type.key(), value.id())
                    }
                    ClosureFieldSource::CallableReferenceReceiver => {
                        FieldIdentityKey::callable_reference_receiver(
                            generated_type.key(),
                            value.id(),
                        )
                    }
                }
                .map_err(ClosureEnvironmentIdentityError::Field)?;
                let field = CborIdentityRecord::from_key(key)
                    .map_err(ClosureEnvironmentIdentityError::Field)?;
                Ok(ClosureFieldIdentity {
                    source,
                    value,
                    field,
                })
            })
            .collect::<Result<Vec<_>, ClosureEnvironmentIdentityError>>()?;
        fields.sort_by_key(|field| field.value.id());
        if fields
            .windows(2)
            .any(|pair| pair[0].value.id() == pair[1].value.id())
        {
            return Err(ClosureEnvironmentIdentityError::DuplicateValue);
        }
        Ok(Self {
            generated_type,
            fields,
        })
    }

    pub const fn generated_type_record(&self) -> &GeneratedTypeRecord {
        &self.generated_type
    }

    pub fn callable(&self) -> CallableMaterialization {
        let GeneratedNominalKey::ClosureEnvironment { callable, .. } = self.generated_type.key()
        else {
            unreachable!("a closure environment identity retains its generated key")
        };
        *callable
    }

    pub fn role(&self) -> ClosureEnvironmentRole {
        let GeneratedNominalKey::ClosureEnvironment { role, .. } = self.generated_type.key() else {
            unreachable!("a closure environment identity retains its generated key")
        };
        *role
    }

    pub fn fields(&self) -> &[ClosureFieldIdentity] {
        &self.fields
    }

    pub fn physical_index(&self, source: ClosureFieldSource) -> Option<u32> {
        self.fields
            .iter()
            .position(|field| field.source == source)
            .and_then(|index| u32::try_from(index).ok())
    }
}

/// Typed physical location of one source closure environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureEnvironment {
    class: ClosureClassId,
    identity: ClosureEnvironmentIdentity,
}

impl ClosureEnvironment {
    pub fn checked(
        class: ClosureClassId,
        definition: &ClosureClass,
        identity: ClosureEnvironmentIdentity,
    ) -> Option<Self> {
        (definition.captures.len() == identity.fields.len()).then_some(Self { class, identity })
    }

    pub const fn class(&self) -> ClosureClassId {
        self.class
    }

    pub const fn identity(&self) -> &ClosureEnvironmentIdentity {
        &self.identity
    }
}

#[derive(Debug)]
pub enum ClosureEnvironmentIdentityError {
    DuplicateSource,
    DuplicateValue,
    NonContiguousCaptures,
    ReceiverRoleMismatch,
    MaterializationContextMismatch,
    GeneratedType(GeneratedNominalIdentityError),
    Field(FieldIdentityError),
}

impl fmt::Display for ClosureEnvironmentIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSource => formatter.write_str("closure field source is duplicated"),
            Self::DuplicateValue => {
                formatter.write_str("closure captures the same persistent value more than once")
            }
            Self::NonContiguousCaptures => formatter.write_str(
                "closure capture declaration indices must be contiguous and start at zero",
            ),
            Self::ReceiverRoleMismatch => formatter
                .write_str("only a callable-reference closure may have a bound receiver field"),
            Self::MaterializationContextMismatch => formatter.write_str(
                "closure field value and environment must share a materialization context",
            ),
            Self::GeneratedType(error) => error.fmt(formatter),
            Self::Field(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ClosureEnvironmentIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CallableMaterializationContext,
        CallableTemplateOwner, CanonicalIdentifier, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, GeneratedCallableKey, LexicalCallableParent, LexicalCallableRole,
        LocalValueSelector, PackagePath, PersistentCallableApplicationId, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite, StructuralDefinitionPath,
        StructuralDefinitionSiteRole, StructuralPathSegment,
    };

    use super::*;

    fn source_function(name: &str) -> PersistentFunctionId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
    }

    fn lambda_materialization(context: CallableMaterializationContext) -> CallableMaterialization {
        let key = GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(source_function("closureOwner")),
            role: LexicalCallableRole::LambdaBody,
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
                [],
            ),
        };
        generated_materialization(key, context)
    }

    fn callable_reference_materialization(
        context: CallableMaterializationContext,
    ) -> CallableMaterialization {
        let key = GeneratedCallableKey::CallableReferenceInvoke {
            parent: LexicalCallableParent::function(source_function("referenceOwner")),
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::CallableConversion, 0),
                [],
            ),
        };
        generated_materialization(key, context)
    }

    fn generated_materialization(
        key: GeneratedCallableKey,
        context: CallableMaterializationContext,
    ) -> CallableMaterialization {
        let callable = scoop_identity::PersistentGeneratedCallableId::from_key(&key).unwrap();
        CallableMaterialization::new(CallableTemplateOwner::Generated(callable), context)
    }

    fn value(context: CallableMaterializationContext, declaration_index: u32) -> LocalValueRecord {
        CborIdentityRecord::from_key(LocalValueKey::new(
            CallableMaterialization::new(
                CallableTemplateOwner::Function(source_function("valueOwner")),
                context,
            ),
            LocalValueSelector::Parameter { declaration_index },
        ))
        .unwrap()
    }

    #[test]
    fn physical_fields_follow_persistent_value_order_not_declaration_order() {
        let callable = lambda_materialization(CallableMaterializationContext::NoSubstitution);
        let first = value(CallableMaterializationContext::NoSubstitution, 0);
        let second = value(CallableMaterializationContext::NoSubstitution, 1);
        let identity = ClosureEnvironmentIdentity::for_lambda(
            callable,
            vec![
                (
                    ClosureFieldSource::Capture {
                        declaration_index: 0,
                    },
                    first.clone(),
                ),
                (
                    ClosureFieldSource::Capture {
                        declaration_index: 1,
                    },
                    second.clone(),
                ),
            ],
        )
        .unwrap();

        let expected = if first.id() < second.id() {
            vec![first.id(), second.id()]
        } else {
            vec![second.id(), first.id()]
        };
        assert_eq!(
            identity
                .fields()
                .iter()
                .map(|field| field.value_record().id())
                .collect::<Vec<_>>(),
            expected
        );
        for field in identity.fields() {
            assert_eq!(
                field.field_record().key(),
                &FieldIdentityKey::closure_capture(
                    identity.generated_type_record().key(),
                    field.value_record().id(),
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn callable_reference_combines_receiver_and_capture_without_losing_roles() {
        let context = CallableMaterializationContext::NoSubstitution;
        let receiver = value(context, 0);
        let capture = value(context, 1);
        let identity = ClosureEnvironmentIdentity::for_callable_reference(
            callable_reference_materialization(context),
            vec![
                (
                    ClosureFieldSource::CallableReferenceReceiver,
                    receiver.clone(),
                ),
                (
                    ClosureFieldSource::Capture {
                        declaration_index: 0,
                    },
                    capture,
                ),
            ],
        )
        .unwrap();

        let receiver_index = identity
            .physical_index(ClosureFieldSource::CallableReferenceReceiver)
            .unwrap();
        let receiver_field = &identity.fields()[receiver_index as usize];
        assert_eq!(receiver_field.value_record(), &receiver);
        assert_eq!(
            receiver_field.field_record().key(),
            &FieldIdentityKey::callable_reference_receiver(
                identity.generated_type_record().key(),
                receiver.id(),
            )
            .unwrap()
        );
    }

    #[test]
    fn closure_fields_reject_a_different_materialization_context() {
        let owner = source_function("applicationOwner");
        let application = PersistentCallableApplicationId::from_key(
            &CallableApplicationKey::for_function(owner, CallableInstantiationOwner::NoOwner),
        )
        .unwrap();
        let identity = ClosureEnvironmentIdentity::for_lambda(
            lambda_materialization(CallableMaterializationContext::Application(application)),
            vec![(
                ClosureFieldSource::Capture {
                    declaration_index: 0,
                },
                value(CallableMaterializationContext::NoSubstitution, 0),
            )],
        );

        assert!(matches!(
            identity,
            Err(ClosureEnvironmentIdentityError::MaterializationContextMismatch)
        ));
    }

    #[test]
    fn lambda_environment_rejects_a_callable_reference_receiver() {
        let context = CallableMaterializationContext::NoSubstitution;
        let identity = ClosureEnvironmentIdentity::for_lambda(
            lambda_materialization(context),
            vec![(
                ClosureFieldSource::CallableReferenceReceiver,
                value(context, 0),
            )],
        );

        assert!(matches!(
            identity,
            Err(ClosureEnvironmentIdentityError::ReceiverRoleMismatch)
        ));
    }
}
