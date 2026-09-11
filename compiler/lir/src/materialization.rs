//! Persistent ownership of physical entities first introduced by LIR.

use scoop_identity::{
    CborIdentityRecord, OdrGroupId, OdrMemberDiscriminator, OdrMemberId, OdrMemberKey,
    OdrMemberRole, PersistentExactTypeId, SpecializationKey,
};

type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// The unique materialization root inherited or introduced by LIR.
///
/// The variants distinguish Cone ownership from ODR ownership and, for an
/// ODR root, retain which stage owns the group record. This prevents LIR from
/// re-emitting a HIR/MIR group or omitting a structural group that first
/// becomes necessary during physical lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationRoot(MaterializationRootKind);

#[derive(Clone, Debug, Eq, PartialEq)]
enum MaterializationRootKind {
    ConeOwned,
    PriorStageOdr(OdrGroupId),
    LirStructuralOdr(OdrGroupRecord),
}

impl MaterializationRoot {
    pub const fn cone_owned() -> Self {
        Self(MaterializationRootKind::ConeOwned)
    }

    pub const fn prior_stage_odr(group: OdrGroupId) -> Self {
        Self(MaterializationRootKind::PriorStageOdr(group))
    }

    pub fn lir_structural_odr(
        exact_type: PersistentExactTypeId,
    ) -> Result<Self, scoop_wire::HashError> {
        CborIdentityRecord::from_key(SpecializationKey::StructuralType { exact_type })
            .map(|record| Self(MaterializationRootKind::LirStructuralOdr(record)))
    }

    pub const fn odr_group_id(&self) -> Option<OdrGroupId> {
        match &self.0 {
            MaterializationRootKind::ConeOwned => None,
            MaterializationRootKind::PriorStageOdr(group) => Some(*group),
            MaterializationRootKind::LirStructuralOdr(record) => Some(record.id()),
        }
    }

    fn materialization(
        &self,
        role: OdrMemberRole,
        discriminator: OdrMemberDiscriminator,
    ) -> Result<MaterializationIdentity, scoop_wire::HashError> {
        let group = match &self.0 {
            MaterializationRootKind::ConeOwned => {
                return Ok(MaterializationIdentity(
                    MaterializationIdentityKind::ConeOwned,
                ));
            }
            MaterializationRootKind::PriorStageOdr(group) => OdrGroupProvenance::PriorStage(*group),
            MaterializationRootKind::LirStructuralOdr(record) => {
                OdrGroupProvenance::Lir(record.clone())
            }
        };
        let key = OdrMemberKey::new(group.id(), role, discriminator)
            .expect("the closed LIR entity role/discriminator pair is valid");
        let member = CborIdentityRecord::from_key(key)?;
        Ok(MaterializationIdentity(
            MaterializationIdentityKind::OdrOwned(Box::new(OdrMaterializationIdentity {
                group,
                member,
            })),
        ))
    }

    pub(crate) fn layout(
        &self,
        layout: scoop_identity::PersistentLayoutId,
    ) -> Result<MaterializationIdentity, scoop_wire::HashError> {
        self.materialization(
            OdrMemberRole::Layout,
            OdrMemberDiscriminator::Layout(layout),
        )
    }

    pub(crate) fn scan(
        &self,
        scan: scoop_identity::PersistentScanId,
    ) -> Result<MaterializationIdentity, scoop_wire::HashError> {
        self.materialization(
            OdrMemberRole::ScanProgram,
            OdrMemberDiscriminator::Scan(scan),
        )
    }

    pub(crate) fn type_descriptor(
        &self,
        exact_type: PersistentExactTypeId,
    ) -> Result<MaterializationIdentity, scoop_wire::HashError> {
        self.materialization(
            OdrMemberRole::TypeDescriptor,
            OdrMemberDiscriminator::ExactType(exact_type),
        )
    }

    pub(crate) fn static_storage(
        &self,
        storage: scoop_identity::PersistentStaticStorageId,
    ) -> Result<MaterializationIdentity, scoop_wire::HashError> {
        self.materialization(
            OdrMemberRole::StaticStorage,
            OdrMemberDiscriminator::StaticStorage(storage),
        )
    }

    pub(crate) fn immortal_object(
        &self,
        object: scoop_identity::PersistentImmortalObjectId,
    ) -> Result<MaterializationIdentity, scoop_wire::HashError> {
        self.materialization(
            OdrMemberRole::ImmortalObject,
            OdrMemberDiscriminator::ImmortalObject(object),
        )
    }
}

/// Persistent ownership relation for one physical LIR entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationIdentity(MaterializationIdentityKind);

#[derive(Clone, Debug, Eq, PartialEq)]
enum MaterializationIdentityKind {
    ConeOwned,
    OdrOwned(Box<OdrMaterializationIdentity>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OdrMaterializationIdentity {
    group: OdrGroupProvenance,
    member: OdrMemberRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OdrGroupProvenance {
    PriorStage(OdrGroupId),
    /// Repeated entities under the same structural root carry the same
    /// record; foundation projection canonicalizes that relation by group id.
    Lir(OdrGroupRecord),
}

impl OdrGroupProvenance {
    pub const fn id(&self) -> OdrGroupId {
        match self {
            Self::PriorStage(id) => *id,
            Self::Lir(record) => record.id(),
        }
    }
}

impl MaterializationIdentity {
    fn sibling(
        &self,
        role: OdrMemberRole,
        discriminator: OdrMemberDiscriminator,
    ) -> Result<Self, scoop_wire::HashError> {
        let group = match &self.0 {
            MaterializationIdentityKind::ConeOwned => {
                return Ok(Self(MaterializationIdentityKind::ConeOwned));
            }
            MaterializationIdentityKind::OdrOwned(identity) => identity.group.clone(),
        };
        let key = OdrMemberKey::new(group.id(), role, discriminator)
            .expect("the closed LIR entity role/discriminator pair is valid");
        let member = CborIdentityRecord::from_key(key)?;
        Ok(Self(MaterializationIdentityKind::OdrOwned(Box::new(
            OdrMaterializationIdentity { group, member },
        ))))
    }

    pub(crate) fn dispatch_table(
        &self,
        table: scoop_identity::PersistentDispatchTableId,
    ) -> Result<Self, scoop_wire::HashError> {
        self.sibling(
            OdrMemberRole::DispatchTable,
            OdrMemberDiscriminator::DispatchTable(table),
        )
    }

    pub const fn is_cone_owned(&self) -> bool {
        matches!(&self.0, MaterializationIdentityKind::ConeOwned)
    }

    pub const fn lir_odr_group_record(&self) -> Option<&OdrGroupRecord> {
        match &self.0 {
            MaterializationIdentityKind::ConeOwned => None,
            MaterializationIdentityKind::OdrOwned(identity) => match &identity.group {
                OdrGroupProvenance::PriorStage(_) => None,
                OdrGroupProvenance::Lir(group) => Some(group),
            },
        }
    }

    pub const fn odr_member_record(&self) -> Option<&OdrMemberRecord> {
        match &self.0 {
            MaterializationIdentityKind::ConeOwned => None,
            MaterializationIdentityKind::OdrOwned(identity) => Some(&identity.member),
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CoreBuiltinNominal, ExactTypeKey, LayoutKey, NonEmptyVec, RepresentationRole, ScanKey,
        ScanRole,
    };

    use super::*;
    use crate::LirTargetProfile;

    fn exact() -> PersistentExactTypeId {
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        PersistentExactTypeId::from_key(&ExactTypeKey::Tuple(NonEmptyVec::from_first(unit, [])))
            .unwrap()
    }

    #[test]
    fn cone_root_never_fabricates_an_odr_relation() {
        let layout = CborIdentityRecord::from_key(LayoutKey::new(
            exact(),
            LirTargetProfile::DARWIN_AARCH64.wire_id(),
            RepresentationRole::ManagedValue,
        ))
        .unwrap();
        let identity = MaterializationRoot::cone_owned()
            .layout(layout.id())
            .unwrap();

        assert!(identity.is_cone_owned());
        assert_eq!(identity.lir_odr_group_record(), None);
        assert_eq!(identity.odr_member_record(), None);
    }

    #[test]
    fn lir_structural_root_emits_its_group_and_each_typed_member() {
        let root = MaterializationRoot::lir_structural_odr(exact()).unwrap();
        let layout = CborIdentityRecord::from_key(LayoutKey::new(
            exact(),
            LirTargetProfile::DARWIN_AARCH64.wire_id(),
            RepresentationRole::ManagedValue,
        ))
        .unwrap();
        let scan =
            CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::InlineValue)).unwrap();

        let layout_identity = root.layout(layout.id()).unwrap();
        let scan_identity = root.scan(scan.id()).unwrap();

        assert_eq!(
            layout_identity.lir_odr_group_record(),
            scan_identity.lir_odr_group_record()
        );
        assert_eq!(
            layout_identity.odr_member_record().unwrap().key().role(),
            OdrMemberRole::Layout
        );
        assert_eq!(
            scan_identity.odr_member_record().unwrap().key().role(),
            OdrMemberRole::ScanProgram
        );
    }

    #[test]
    fn prior_stage_root_emits_only_the_new_member() {
        let group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: exact(),
        })
        .unwrap();
        let layout = CborIdentityRecord::from_key(LayoutKey::new(
            exact(),
            LirTargetProfile::DARWIN_AARCH64.wire_id(),
            RepresentationRole::ManagedValue,
        ))
        .unwrap();
        let identity = MaterializationRoot::prior_stage_odr(group.id())
            .layout(layout.id())
            .unwrap();

        assert_eq!(identity.lir_odr_group_record(), None);
        assert_eq!(
            identity.odr_member_record().unwrap().key().group(),
            group.id()
        );
    }
}
