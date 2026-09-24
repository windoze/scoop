use scoop_hir::*;
use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    FieldIdentityKey, NominalDeclarationOwner, NonEmptyVec, PackagePath, PersistentFieldId,
    PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

pub(super) struct Nominal {
    pub record: NominalInterfaceRecordV1,
    pub fields: [PersistentFieldId; 2],
}

impl Nominal {
    pub fn apply(owner: SourceNominalId, arity: usize) -> SignatureTypeKey {
        match owner {
            SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
            SourceNominalId::GenericTemplate(origin) => SignatureTypeKey::NominalApplication {
                origin,
                arguments: NonEmptyVec::new(
                    (0..arity)
                        .map(|index| SignatureTypeKey::Binder {
                            depth: 0,
                            index: index as u32,
                        })
                        .collect(),
                )
                .unwrap(),
            },
        }
    }

    pub fn applied_type(&self, arity: usize) -> SignatureTypeKey {
        Self::apply(self.record.declaration(), arity)
    }
}

pub(super) fn ordinary_provider() -> ConeIdentity {
    ConeCoordinate::new("fields", "ordinary", "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

pub(super) fn nominal(provider: ConeIdentity, arity: u32) -> Nominal {
    let site = SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let key = SourceDeclarationKey::nominal(site, name("Value"), SourceNominalKind::Struct, arity);
    let owner = if arity == 0 {
        NominalDeclarationOwner::Concrete(PersistentTypeId::from_source_declaration(&key).unwrap())
    } else {
        NominalDeclarationOwner::GenericTemplate(
            PersistentGenericTypeId::from_source_declaration(&key).unwrap(),
        )
    };
    let fields = ["secondName", "firstName"].map(|value| {
        PersistentFieldId::from_key(&FieldIdentityKey::source_declared(&key, name(value)).unwrap())
            .unwrap()
    });
    let value_type = SignatureTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    );
    let shape = StructSourceShapeV1::try_new(
        fields
            .iter()
            .map(|id| NominalSourceFieldV1::new(*id, value_type.clone()))
            .collect(),
        NominalCLayoutPolicyV1::Ordinary,
        false,
    )
    .unwrap();
    let record = crate::nominal_interface_fixture::public_record(
        owner,
        PublicNominalKindV1::Struct,
        CanonicalBinderListV1::try_new(
            (0..arity)
                .map(|index| {
                    TypeParameterBinderV1::new(
                        name(&format!("T{index}")),
                        TypeParameterBoundsV1::Unconstrained,
                    )
                })
                .collect(),
        )
        .unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        NominalSourceShapeV1::Struct(shape),
    )
    .unwrap();
    Nominal { record, fields }
}

pub(super) fn section(nominals: Vec<NominalInterfaceRecordV1>) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(vec![]).unwrap(),
        CanonicalNominalInterfacesV1::try_new(nominals).unwrap(),
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

fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
