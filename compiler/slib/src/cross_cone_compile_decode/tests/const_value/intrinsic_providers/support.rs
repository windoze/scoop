use super::*;
use crate::cross_cone_hir_authority::ValidatedNominalProviderView;
use scoop_hir::{
    IntrinsicTypeParameters, IntrinsicTypeTarget, TypeParameterBinderV1, TypeParameterBoundsV1,
};
use scoop_identity::{ConeCoordinate, PersistentGenericTypeId};

pub(super) struct Provider {
    pub identity: ConeIdentity,
    pub owner: SourceNominalId,
    pub family: IntrinsicTypeKind,
    pub interface: CrossConeHirInterfaceSectionV1,
}
impl Provider {
    pub fn new(
        identity: ConeIdentity,
        family: IntrinsicTypeKind,
        pending: &mut PendingIdentityValidation,
    ) -> Self {
        let kind = match family.target() {
            IntrinsicTypeTarget::Class => SourceNominalKind::Class,
            IntrinsicTypeTarget::Struct => SourceNominalKind::Struct,
        };
        let bounds = match family.parameters() {
            IntrinsicTypeParameters::None => vec![],
            IntrinsicTypeParameters::OneInvariantUnconstrained => {
                vec![TypeParameterBoundsV1::Unconstrained]
            }
            IntrinsicTypeParameters::OneInvariantValue => vec![TypeParameterBoundsV1::Value],
        };
        let key = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                identity,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("SameName").unwrap(),
            kind,
            bounds.len() as u32,
        );
        let owner = if bounds.is_empty() {
            let record = CborIdentityRecord::<PersistentTypeId, _>::from_key(key).unwrap();
            let owner = SourceNominalId::Concrete(record.id());
            pending
                .register_external_canonical_authority(record)
                .unwrap();
            owner
        } else {
            let record = CborIdentityRecord::<PersistentGenericTypeId, _>::from_key(key).unwrap();
            let owner = SourceNominalId::GenericTemplate(record.id());
            pending
                .register_external_canonical_authority(record)
                .unwrap();
            owner
        };
        let binders = CanonicalBinderListV1::try_new(
            bounds
                .into_iter()
                .map(|bound| {
                    TypeParameterBinderV1::new(CanonicalIdentifier::new("T").unwrap(), bound)
                })
                .collect(),
        )
        .unwrap();
        let shape = NominalSourceShapeV1::Intrinsic(
            scoop_hir::NominalIntrinsicRepresentationV1::new(family),
        );
        Self {
            identity,
            owner,
            family,
            interface: section(record(owner, shape, binders)),
        }
    }

    pub fn replace_shape(&mut self, shape: NominalSourceShapeV1) {
        let binders = self
            .interface
            .nominal_interfaces()
            .get(self.owner)
            .unwrap()
            .type_parameters()
            .clone();
        self.interface = section(record(self.owner, shape, binders));
    }

    pub fn view<'a>(
        &'a self,
        base: &'a ConstValidatedCrossConeHirFrontSections<'_>,
    ) -> ValidatedNominalProviderView<'a> {
        ValidatedNominalProviderView {
            identity: self.identity,
            core: base.hir_core_production(),
            interface: &self.interface,
        }
    }
}

pub(super) fn provider_identity(index: usize) -> ConeIdentity {
    ConeCoordinate::new("example", &format!("intrinsic-provider-{index}"), "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
pub(super) fn concrete(owner: SourceNominalId) -> PersistentTypeId {
    let SourceNominalId::Concrete(id) = owner else {
        panic!("concrete fixture type")
    };
    id
}
pub(super) fn generic(owner: SourceNominalId) -> PersistentGenericTypeId {
    let SourceNominalId::GenericTemplate(id) = owner else {
        panic!("generic fixture type")
    };
    id
}
pub(super) fn with_base(run: impl FnOnce(&ConstValidatedCrossConeHirFrontSections<'_>)) {
    let bytes = cross_cone_artifact(empty_cross_cone_hir_interface());
    let base = validate_until_source_interfaces(&bytes)
        .validate_const_values(vec![])
        .unwrap();
    run(&base);
}
fn record(
    owner: SourceNominalId,
    shape: NominalSourceShapeV1,
    binders: CanonicalBinderListV1,
) -> NominalInterfaceRecordV1 {
    crate::nominal_interface_fixture::public_record(
        owner,
        shape.kind(),
        binders,
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        shape,
    )
    .unwrap()
}
fn section(record: NominalInterfaceRecordV1) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(vec![]).unwrap(),
        CanonicalNominalInterfacesV1::try_new(vec![record]).unwrap(),
        CanonicalCallableInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
        CanonicalExportConstValuesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(vec![]).unwrap(),
    )
}
