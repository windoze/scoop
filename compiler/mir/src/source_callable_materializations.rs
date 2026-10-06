use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOdrMemberId, CallableOwner,
    CallableTemplateOwner, CborIdentityRecord, CoreBuiltinNominal, ExactCallableSignature,
    ExactTypeKey, OdrGroupId, OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError,
    OdrMemberKey, OdrMemberRole,
};
use scoop_wire::HashError;

use crate::{CallableSignatureRecord, CallableSignatureSubject, FunctionId};

pub type SourceCallableOdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceCallableOwner {
    Lexical(Option<SourceCallableOdrMemberRecord>),
    Exact(crate::ExactOwnerRoot),
}

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
    owner: SourceCallableOwner,
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
                    | CallableTemplateOwner::VariantConstructor(_)
                    | CallableTemplateOwner::ReleaseHook(_) => {
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
            owner: SourceCallableOwner::Lexical(odr_member),
            signature: CallableSignatureRecord::new(subject, signature),
        })
    }

    pub fn derived_equality(
        function: FunctionId,
        exact: &crate::SourceExactTypeRecord,
        nominal_group: Option<&crate::SourceNominalSpecializationRecord>,
        signature: ExactCallableSignature,
    ) -> Result<Self, SourceCallableMaterializationError> {
        let generated = scoop_identity::PersistentGeneratedCallableId::from_key(
            &scoop_identity::GeneratedCallableKey::DerivedEquality {
                exact_owner: exact.id(),
            },
        )
        .map_err(SourceCallableMaterializationError::GeneratedCallable)?;
        Self::exact_method(function, exact, nominal_group, signature, generated)
    }

    pub fn tuple_encoding(
        function: FunctionId,
        exact: &crate::SourceExactTypeRecord,
        signature: ExactCallableSignature,
    ) -> Result<Self, SourceCallableMaterializationError> {
        let generated = scoop_identity::PersistentGeneratedCallableId::from_key(
            &scoop_identity::GeneratedCallableKey::TupleEncoding {
                exact_owner: exact.id(),
            },
        )
        .map_err(SourceCallableMaterializationError::GeneratedCallable)?;
        Self::exact_method(function, exact, None, signature, generated)
    }

    fn exact_method(
        function: FunctionId,
        exact: &crate::SourceExactTypeRecord,
        nominal_group: Option<&crate::SourceNominalSpecializationRecord>,
        signature: ExactCallableSignature,
        generated: scoop_identity::PersistentGeneratedCallableId,
    ) -> Result<Self, SourceCallableMaterializationError> {
        let discriminator = OdrMemberDiscriminator::GeneratedCallable(generated);
        let owner = if matches!(exact.key(), ExactTypeKey::Nominal(owner)
            if *owner == CoreBuiltinNominal::Unit.identity_record().id())
        {
            if nominal_group.is_some() {
                return Err(SourceCallableMaterializationError::ExactOwner(
                    crate::ExactOwnerRootError::UnexpectedNominalGroup,
                ));
            }
            // Unit has no source equality body owned by a defining Cone.
            crate::ExactOwnerRoot::structural(exact, OdrMemberRole::CallableBody, discriminator)
        } else {
            crate::ExactOwnerRoot::for_member(
                exact,
                nominal_group,
                OdrMemberRole::CallableBody,
                discriminator,
            )
        }
        .map_err(SourceCallableMaterializationError::ExactOwner)?;
        let subject = match owner.member_record() {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key())
                    .map_err(SourceCallableMaterializationError::OdrMember)?,
            ),
            None => CallableSignatureSubject::strong(CallableOwner::Generated(generated)),
        };
        Ok(Self {
            function,
            materialization: CallableMaterialization::new(
                CallableTemplateOwner::Generated(generated),
                CallableMaterializationContext::NoSubstitution,
            ),
            owner: SourceCallableOwner::Exact(owner),
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
        match &self.owner {
            SourceCallableOwner::Lexical(member) => member.as_ref(),
            SourceCallableOwner::Exact(owner) => owner.member_record(),
        }
    }

    pub const fn exact_owner(&self) -> Option<&crate::ExactOwnerRoot> {
        match &self.owner {
            SourceCallableOwner::Lexical(_) => None,
            SourceCallableOwner::Exact(owner) => Some(owner),
        }
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

    pub fn get_by_materialization(
        &self,
        materialization: CallableMaterialization,
    ) -> Option<&SourceCallableMaterialization> {
        self.entries
            .iter()
            .find(|entry| entry.materialization == materialization)
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
    GeneratedCallable(scoop_identity::GeneratedCallableIdentityError),
    ExactOwner(crate::ExactOwnerRootError),
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
            Self::GeneratedCallable(error) => write!(formatter, "cannot derive equality identity: {error}"),
            Self::ExactOwner(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SourceCallableMaterializationError {}

#[cfg(test)]
mod tests;
