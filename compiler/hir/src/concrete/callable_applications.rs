use std::collections::HashMap;
use std::fmt;

use scoop_identity::{StableIdentityOrderError, stable_topological_identity_order};
use scoop_wire::HashError;

use super::*;

pub type CallableApplicationRecord =
    CborIdentityRecord<PersistentCallableApplicationId, CallableApplicationKey>;
pub type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
pub type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableApplicationOdr {
    group: OdrGroupId,
    body: CallableOdrMemberId,
}

impl CallableApplicationOdr {
    pub const fn group(self) -> OdrGroupId {
        self.group
    }

    pub const fn body(self) -> CallableOdrMemberId {
        self.body
    }
}

/// Complete canonical application table referenced by LocalConcrete HIR.
/// Records are dependency-first and every enclosing application is present.
#[derive(Clone, Debug)]
pub struct CallableApplicationIdentities {
    records: Vec<CallableApplicationRecord>,
    positions: HashMap<PersistentCallableApplicationId, usize>,
    odr: HashMap<PersistentCallableApplicationId, CallableApplicationOdr>,
    generated_bodies: HashMap<
        (
            PersistentCallableApplicationId,
            PersistentGeneratedCallableId,
        ),
        CallableOdrMemberId,
    >,
    odr_groups: Vec<OdrGroupRecord>,
    odr_members: Vec<OdrMemberRecord>,
}

impl CallableApplicationIdentities {
    pub fn checked(
        records: Vec<CallableApplicationRecord>,
    ) -> Result<Self, CallableApplicationIdentityError> {
        Self::checked_with_generated_bodies(records, [])
    }

    pub fn checked_with_generated_bodies(
        records: Vec<CallableApplicationRecord>,
        generated_bodies: impl IntoIterator<
            Item = (
                PersistentCallableApplicationId,
                PersistentGeneratedCallableId,
            ),
        >,
    ) -> Result<Self, CallableApplicationIdentityError> {
        let records =
            stable_topological_identity_order(records, CborIdentityRecord::id, |record| {
                record.key().callable_application_dependencies()
            })
            .map_err(CallableApplicationIdentityError::ApplicationOrder)?;
        let positions = records
            .iter()
            .enumerate()
            .map(|(position, record)| (record.id(), position))
            .collect();
        let mut odr = HashMap::with_capacity(records.len());
        let mut odr_groups = Vec::with_capacity(records.len());
        let mut odr_members = Vec::with_capacity(records.len());
        for application in &records {
            let group = CborIdentityRecord::from_key(SpecializationKey::Callable {
                application: application.key().clone(),
            })
            .map_err(CallableApplicationIdentityError::OdrGroup)?;
            let member_key = OdrMemberKey::new(
                group.id(),
                OdrMemberRole::CallableBody,
                OdrMemberDiscriminator::CallableApplication(application.id()),
            )
            .map_err(CallableApplicationIdentityError::OdrMember)?;
            let body = CallableOdrMemberId::from_key(&member_key)
                .map_err(CallableApplicationIdentityError::OdrMember)?;
            let member = CborIdentityRecord::from_key(member_key)
                .map_err(CallableApplicationIdentityError::OdrMemberRecord)?;
            debug_assert_eq!(body.member(), member.id());
            odr.insert(
                application.id(),
                CallableApplicationOdr {
                    group: group.id(),
                    body,
                },
            );
            odr_groups.push(group);
            odr_members.push(member);
        }
        let mut member_positions = odr_members
            .iter()
            .enumerate()
            .map(|(position, record)| (record.id(), position))
            .collect::<HashMap<_, _>>();
        let mut generated_body_members = HashMap::new();
        for (application, generated) in generated_bodies {
            let application_odr = odr.get(&application).copied().ok_or(
                CallableApplicationIdentityError::MissingApplication(application),
            )?;
            let member_key = OdrMemberKey::new(
                application_odr.group(),
                OdrMemberRole::CallableBody,
                OdrMemberDiscriminator::GeneratedCallable(generated),
            )
            .map_err(CallableApplicationIdentityError::OdrMember)?;
            let body = CallableOdrMemberId::from_key(&member_key)
                .map_err(CallableApplicationIdentityError::OdrMember)?;
            let member = CborIdentityRecord::from_key(member_key)
                .map_err(CallableApplicationIdentityError::OdrMemberRecord)?;
            debug_assert_eq!(body.member(), member.id());
            if let Some(position) = member_positions.get(&member.id()).copied() {
                if odr_members[position] != member {
                    return Err(CallableApplicationIdentityError::OdrMemberCollision(
                        member.id(),
                    ));
                }
            } else {
                member_positions.insert(member.id(), odr_members.len());
                odr_members.push(member);
            }
            generated_body_members.insert((application, generated), body);
        }
        odr_groups.sort_by_key(CborIdentityRecord::id);
        odr_members.sort_by_key(CborIdentityRecord::id);
        Ok(Self {
            records,
            positions,
            odr,
            generated_bodies: generated_body_members,
            odr_groups,
            odr_members,
        })
    }

    pub fn get(&self, id: PersistentCallableApplicationId) -> Option<&CallableApplicationRecord> {
        self.positions
            .get(&id)
            .map(|position| &self.records[*position])
    }

    pub fn records(&self) -> &[CallableApplicationRecord] {
        &self.records
    }

    pub fn odr(&self, id: PersistentCallableApplicationId) -> Option<CallableApplicationOdr> {
        self.odr.get(&id).copied()
    }

    pub fn generated_body(
        &self,
        application: PersistentCallableApplicationId,
        generated: PersistentGeneratedCallableId,
    ) -> Option<CallableOdrMemberId> {
        self.generated_bodies
            .get(&(application, generated))
            .copied()
    }

    pub fn odr_group_records(&self) -> &[OdrGroupRecord] {
        &self.odr_groups
    }

    pub fn odr_member_records(&self) -> &[OdrMemberRecord] {
        &self.odr_members
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableApplicationIdentityError {
    ApplicationOrder(StableIdentityOrderError<PersistentCallableApplicationId>),
    OdrGroup(HashError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
    MissingApplication(PersistentCallableApplicationId),
    OdrMemberCollision(OdrMemberId),
}

impl fmt::Display for CallableApplicationIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApplicationOrder(error) => error.fmt(formatter),
            Self::OdrGroup(error) => write!(formatter, "failed to derive ODR group: {error}"),
            Self::OdrMember(error) => write!(formatter, "failed to derive ODR member: {error}"),
            Self::OdrMemberRecord(error) => {
                write!(formatter, "failed to record ODR member: {error}")
            }
            Self::MissingApplication(_) => {
                formatter.write_str("generated callable references a missing application")
            }
            Self::OdrMemberCollision(_) => {
                formatter.write_str("distinct ODR member keys have the same identity")
            }
        }
    }
}

impl std::error::Error for CallableApplicationIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableInstantiationOwner, CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactTypeKey, GeneratedCallableKey, PackagePath,
        PersistentExactTypeId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;

    fn function(name: &str) -> PersistentFunctionId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
        .id()
    }

    fn unit() -> PersistentExactTypeId {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
        .id()
    }

    #[test]
    fn applications_are_dependency_ordered_and_require_enclosing_records() {
        let outer_key = CallableApplicationKey::for_function(
            function("outer"),
            CallableInstantiationOwner::ExactNominalOwner(unit()),
        );
        let outer = CborIdentityRecord::from_key(outer_key).unwrap();
        let inner = CborIdentityRecord::from_key(CallableApplicationKey::for_function(
            function("inner"),
            CallableInstantiationOwner::EnclosingCallableApplication(outer.id()),
        ))
        .unwrap();

        let generated = CborIdentityRecord::from_key(GeneratedCallableKey::DerivedEquality {
            exact_owner: unit(),
        })
        .unwrap()
        .id();
        let table = CallableApplicationIdentities::checked_with_generated_bodies(
            vec![inner.clone(), outer.clone()],
            [(outer.id(), generated)],
        )
        .expect("the complete application graph is valid");
        assert_eq!(table.records(), &[outer.clone(), inner.clone()]);
        assert_eq!(table.get(inner.id()), Some(&inner));

        for application in [&outer, &inner] {
            let odr = table
                .odr(application.id())
                .expect("every callable application has one ODR body member");
            let group = table
                .odr_group_records()
                .iter()
                .find(|record| record.id() == odr.group())
                .expect("the ODR group record is present");
            assert!(matches!(
                group.key(),
                SpecializationKey::Callable { application: key }
                    if key == application.key()
            ));
            let member = table
                .odr_member_records()
                .iter()
                .find(|record| record.id() == odr.body().member())
                .expect("the ODR body member record is present");
            assert_eq!(member.key().group(), group.id());
            assert_eq!(member.key().role(), OdrMemberRole::CallableBody);
            assert!(matches!(
                member.key().discriminator(),
                OdrMemberDiscriminator::CallableApplication(id) if *id == application.id()
            ));
        }
        assert_eq!(table.odr_group_records().len(), 2);
        assert_eq!(table.odr_member_records().len(), 3);
        let generated_body = table
            .generated_body(outer.id(), generated)
            .expect("the generated callable has one body member in its application group");
        let generated_member = table
            .odr_member_records()
            .iter()
            .find(|record| record.id() == generated_body.member())
            .expect("the generated callable ODR member record is present");
        assert_eq!(
            generated_member.key().group(),
            table.odr(outer.id()).unwrap().group()
        );
        assert!(matches!(
            generated_member.key().discriminator(),
            OdrMemberDiscriminator::GeneratedCallable(id) if *id == generated
        ));

        assert!(matches!(
            CallableApplicationIdentities::checked(vec![inner.clone()]),
            Err(CallableApplicationIdentityError::ApplicationOrder(
                StableIdentityOrderError::MissingDependency { .. }
            ))
        ));
        assert!(matches!(
            CallableApplicationIdentities::checked_with_generated_bodies(
                vec![outer],
                [(inner.id(), generated)]
            ),
            Err(CallableApplicationIdentityError::MissingApplication(id)) if id == inner.id()
        ));
    }
}
