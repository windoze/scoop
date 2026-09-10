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
    odr_groups: Vec<OdrGroupRecord>,
    odr_members: Vec<OdrMemberRecord>,
}

impl CallableApplicationIdentities {
    pub fn checked(
        records: Vec<CallableApplicationRecord>,
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
        odr_groups.sort_by_key(CborIdentityRecord::id);
        odr_members.sort_by_key(CborIdentityRecord::id);
        Ok(Self {
            records,
            positions,
            odr,
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
        }
    }
}

impl std::error::Error for CallableApplicationIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableInstantiationOwner, CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactTypeKey, PackagePath, PersistentExactTypeId,
        PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
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

        let table = CallableApplicationIdentities::checked(vec![inner.clone(), outer.clone()])
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
        assert_eq!(table.odr_member_records().len(), 2);

        assert!(matches!(
            CallableApplicationIdentities::checked(vec![inner]),
            Err(CallableApplicationIdentityError::ApplicationOrder(
                StableIdentityOrderError::MissingDependency { .. }
            ))
        ));
    }
}
