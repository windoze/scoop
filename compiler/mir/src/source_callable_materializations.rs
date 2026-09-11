use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOdrMemberId, CallableOwner,
    CallableTemplateOwner, CborIdentityRecord, ExactCallableSignature, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole,
};
use scoop_wire::HashError;

use crate::{CallableSignatureRecord, CallableSignatureSubject, FunctionId};

pub type SourceCallableOdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// One callable materialization from LocalConcrete HIR at its MIR function
/// location.
///
/// The identity remains HIR-owned. MIR retains this typed relation so later
/// transforms can identify source callables without recovering them from
/// display names, symbols, or arena ordinals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCallableMaterialization {
    function: FunctionId,
    materialization: CallableMaterialization,
    odr_member: Option<SourceCallableOdrMemberRecord>,
    signature: CallableSignatureRecord,
}

impl SourceCallableMaterialization {
    pub fn new(
        function: FunctionId,
        materialization: CallableMaterialization,
        signature: ExactCallableSignature,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, SourceCallableMaterializationError> {
        let (subject, odr_member) = match materialization.context() {
            CallableMaterializationContext::NoSubstitution => {
                if odr_group.is_some() {
                    return Err(SourceCallableMaterializationError::UnexpectedOdrGroup);
                }
                let owner = match materialization.template() {
                    CallableTemplateOwner::Function(id) => CallableOwner::Function(id),
                    CallableTemplateOwner::Constructor(id) => CallableOwner::Constructor(id),
                    CallableTemplateOwner::Accessor(id) => CallableOwner::Accessor(id),
                    CallableTemplateOwner::Generated(id) => CallableOwner::Generated(id),
                    CallableTemplateOwner::GenericFunction(_)
                    | CallableTemplateOwner::VariantConstructor(_) => {
                        return Err(SourceCallableMaterializationError::InvalidStrongTemplate);
                    }
                };
                (CallableSignatureSubject::strong(owner), None)
            }
            CallableMaterializationContext::Application(application) => {
                let group = odr_group.ok_or(SourceCallableMaterializationError::MissingOdrGroup)?;
                let discriminator = match materialization.template() {
                    CallableTemplateOwner::Generated(generated) => {
                        OdrMemberDiscriminator::GeneratedCallable(generated)
                    }
                    _ => OdrMemberDiscriminator::CallableApplication(application),
                };
                let member = callable_member(group, discriminator)?;
                let subject = CallableSignatureSubject::odr(
                    CallableOdrMemberId::from_key(member.key())
                        .map_err(SourceCallableMaterializationError::OdrMember)?,
                );
                (subject, Some(member))
            }
            CallableMaterializationContext::InitializationApplication(_) => {
                let CallableTemplateOwner::Generated(generated) = materialization.template() else {
                    return Err(SourceCallableMaterializationError::InvalidInitializationTemplate);
                };
                let group = odr_group.ok_or(SourceCallableMaterializationError::MissingOdrGroup)?;
                let member =
                    callable_member(group, OdrMemberDiscriminator::GeneratedCallable(generated))?;
                let subject = CallableSignatureSubject::odr(
                    CallableOdrMemberId::from_key(member.key())
                        .map_err(SourceCallableMaterializationError::OdrMember)?,
                );
                (subject, Some(member))
            }
        };
        Ok(Self {
            function,
            materialization,
            odr_member,
            signature: CallableSignatureRecord::new(subject, signature),
        })
    }

    pub const fn function(&self) -> FunctionId {
        self.function
    }

    pub const fn materialization(&self) -> CallableMaterialization {
        self.materialization
    }

    pub const fn odr_member_record(&self) -> Option<&SourceCallableOdrMemberRecord> {
        self.odr_member.as_ref()
    }

    pub const fn signature_record(&self) -> &CallableSignatureRecord {
        &self.signature
    }

    fn sort_key(&self) -> u32 {
        self.function.into_raw().into_u32()
    }
}

fn callable_member(
    group: OdrGroupId,
    discriminator: OdrMemberDiscriminator,
) -> Result<SourceCallableOdrMemberRecord, SourceCallableMaterializationError> {
    let key = OdrMemberKey::new(group, OdrMemberRole::CallableBody, discriminator)
        .map_err(SourceCallableMaterializationError::OdrMember)?;
    CborIdentityRecord::from_key(key).map_err(SourceCallableMaterializationError::OdrMemberRecord)
}

/// Complete one-to-one relation for LocalConcrete HIR callables transposed
/// into this MIR module.
///
/// MIR-generated functions do not enter this relation. They retain their own
/// generated callable identities in the metadata owned by their transform.
#[derive(Clone, Debug, Default)]
pub struct SourceCallableMaterializations {
    entries: Vec<SourceCallableMaterialization>,
}

impl SourceCallableMaterializations {
    pub fn checked(
        mut entries: Vec<SourceCallableMaterialization>,
    ) -> Result<Self, SourceCallableMaterializationRelationError> {
        entries.sort_by_key(SourceCallableMaterialization::sort_key);
        if let Some((first, _)) = entries
            .windows(2)
            .enumerate()
            .find(|(_, pair)| pair[0].function == pair[1].function)
        {
            return Err(
                SourceCallableMaterializationRelationError::DuplicateFunction {
                    first,
                    index: first + 1,
                },
            );
        }
        for (index, entry) in entries.iter().enumerate() {
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.materialization == entry.materialization)
            {
                return Err(
                    SourceCallableMaterializationRelationError::DuplicateMaterialization {
                        first,
                        index,
                    },
                );
            }
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.signature.subject() == entry.signature.subject())
            {
                return Err(
                    SourceCallableMaterializationRelationError::DuplicateSignatureSubject {
                        first,
                        index,
                    },
                );
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, function: FunctionId) -> Option<&SourceCallableMaterialization> {
        let key = function.into_raw().into_u32();
        self.entries
            .binary_search_by_key(&key, SourceCallableMaterialization::sort_key)
            .ok()
            .map(|index| &self.entries[index])
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SourceCallableMaterialization> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceCallableMaterializationRelationError {
    DuplicateFunction { first: usize, index: usize },
    DuplicateMaterialization { first: usize, index: usize },
    DuplicateSignatureSubject { first: usize, index: usize },
}

impl std::fmt::Display for SourceCallableMaterializationRelationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateFunction { first, index } => write!(
                formatter,
                "source callable entries {first} and {index} have the same MIR function"
            ),
            Self::DuplicateMaterialization { first, index } => write!(
                formatter,
                "source callable entries {first} and {index} have the same materialization"
            ),
            Self::DuplicateSignatureSubject { first, index } => write!(
                formatter,
                "source callable entries {first} and {index} have the same signature subject"
            ),
        }
    }
}

impl std::error::Error for SourceCallableMaterializationRelationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceCallableMaterializationError {
    MissingOdrGroup,
    UnexpectedOdrGroup,
    InvalidStrongTemplate,
    InvalidInitializationTemplate,
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl std::fmt::Display for SourceCallableMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingOdrGroup => {
                formatter.write_str("a substituted source callable requires an ODR group")
            }
            Self::UnexpectedOdrGroup => formatter
                .write_str("an unsubstituted source callable cannot belong to an ODR group"),
            Self::InvalidStrongTemplate => formatter.write_str(
                "a strong source callable requires a concrete function, constructor, accessor, or generated template",
            ),
            Self::InvalidInitializationTemplate => formatter.write_str(
                "an initialization application source callable requires a generated template",
            ),
            Self::OdrMember(error) => write!(formatter, "invalid callable ODR member: {error}"),
            Self::OdrMemberRecord(error) => {
                write!(formatter, "cannot derive callable ODR member identity: {error}")
            }
        }
    }
}

impl std::error::Error for SourceCallableMaterializationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CallableMaterializationContext,
        CallableTemplateOwner, CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, Effect, ExactTypeKey, PackagePath,
        PersistentCallableApplicationId, PersistentExactTypeId, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite, SpecializationKey,
    };

    use super::*;

    fn materialization(name: &str) -> CallableMaterialization {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        CallableMaterialization::new(
            CallableTemplateOwner::Function(
                PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
            ),
            CallableMaterializationContext::NoSubstitution,
        )
    }

    fn exact_unit() -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }

    fn signature() -> ExactCallableSignature {
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_unit())
    }

    fn direct(
        function: FunctionId,
        materialization: CallableMaterialization,
    ) -> SourceCallableMaterialization {
        SourceCallableMaterialization::new(function, materialization, signature(), None).unwrap()
    }

    fn applied(
        source: CallableMaterialization,
    ) -> (
        CallableMaterialization,
        CborIdentityRecord<OdrGroupId, SpecializationKey>,
    ) {
        let CallableTemplateOwner::Function(owner) = source.template() else {
            unreachable!()
        };
        let application_key = CallableApplicationKey::for_function(
            owner,
            CallableInstantiationOwner::ExactNominalOwner(exact_unit()),
        );
        let application = PersistentCallableApplicationId::from_key(&application_key).unwrap();
        (
            CallableMaterialization::new(
                source.template(),
                CallableMaterializationContext::Application(application),
            ),
            CborIdentityRecord::from_key(SpecializationKey::Callable {
                application: application_key,
            })
            .unwrap(),
        )
    }

    #[test]
    fn relation_sorts_and_queries_typed_function_locations() {
        let first = FunctionId::from_raw(1_u32.into());
        let second = FunctionId::from_raw(4_u32.into());
        let first_materialization = materialization("first");
        let relation = SourceCallableMaterializations::checked(vec![
            direct(second, materialization("second")),
            direct(first, first_materialization),
        ])
        .unwrap();

        assert_eq!(relation.len(), 2);
        assert_eq!(relation.iter().next().unwrap().function(), first);
        assert_eq!(
            relation.get(first).unwrap().materialization(),
            first_materialization
        );
        assert_eq!(
            relation.get(first).unwrap().signature_record().signature(),
            &signature()
        );
        assert!(matches!(
            relation.get(first).unwrap().signature_record().subject(),
            CallableSignatureSubject::Strong(CallableOwner::Function(_))
        ));
    }

    #[test]
    fn substituted_callable_uses_its_callable_body_member_as_signature_subject() {
        let (materialization, group) = applied(materialization("applied"));
        let CallableMaterializationContext::Application(application) = materialization.context()
        else {
            unreachable!()
        };
        let entry = SourceCallableMaterialization::new(
            FunctionId::from_raw(2_u32.into()),
            materialization,
            signature(),
            Some(group.id()),
        )
        .unwrap();

        let member = entry.odr_member_record().unwrap();
        assert_eq!(member.key().group(), group.id());
        assert!(matches!(
            member.key().discriminator(),
            OdrMemberDiscriminator::CallableApplication(found) if *found == application
        ));
        assert_eq!(
            entry.signature_record().subject(),
            CallableSignatureSubject::odr(CallableOdrMemberId::from_key(member.key()).unwrap())
        );
    }

    #[test]
    fn materialization_context_requires_exactly_its_odr_group_shape() {
        let source = materialization("context");
        let (application, group) = applied(source);
        assert_eq!(
            SourceCallableMaterialization::new(
                FunctionId::from_raw(0_u32.into()),
                source,
                signature(),
                Some(group.id()),
            )
            .unwrap_err(),
            SourceCallableMaterializationError::UnexpectedOdrGroup
        );
        assert_eq!(
            SourceCallableMaterialization::new(
                FunctionId::from_raw(0_u32.into()),
                application,
                signature(),
                None,
            )
            .unwrap_err(),
            SourceCallableMaterializationError::MissingOdrGroup
        );
    }

    #[test]
    fn relation_rejects_duplicate_functions_and_materializations() {
        let first = FunctionId::from_raw(1_u32.into());
        let second = FunctionId::from_raw(4_u32.into());
        let shared = materialization("shared");
        assert_eq!(
            SourceCallableMaterializations::checked(vec![
                direct(first, materialization("first")),
                direct(first, materialization("second")),
            ])
            .unwrap_err(),
            SourceCallableMaterializationRelationError::DuplicateFunction { first: 0, index: 1 }
        );
        assert_eq!(
            SourceCallableMaterializations::checked(vec![
                direct(first, shared),
                direct(second, shared),
            ])
            .unwrap_err(),
            SourceCallableMaterializationRelationError::DuplicateMaterialization {
                first: 0,
                index: 1,
            }
        );

        let (first_application, group) = applied(materialization("application"));
        let application = first_application.context();
        let second_application =
            CallableMaterialization::new(materialization("other").template(), application);
        assert_eq!(
            SourceCallableMaterializations::checked(vec![
                SourceCallableMaterialization::new(
                    first,
                    first_application,
                    signature(),
                    Some(group.id()),
                )
                .unwrap(),
                SourceCallableMaterialization::new(
                    second,
                    second_application,
                    signature(),
                    Some(group.id()),
                )
                .unwrap(),
            ])
            .unwrap_err(),
            SourceCallableMaterializationRelationError::DuplicateSignatureSubject {
                first: 0,
                index: 1,
            }
        );
    }
}
