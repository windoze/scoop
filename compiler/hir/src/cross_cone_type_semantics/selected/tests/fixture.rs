use super::*;

mod uses;

pub(super) struct Fixture {
    pub provider: ConeIdentity,
    pub alternate: ConeIdentity,
    pub owner: PersistentExactTypeId,
    pub derived: PersistentExactTypeId,
    pub enumeration: PersistentExactTypeId,
    pub interface: PersistentExactTypeId,
    pub object_exact: PersistentExactTypeId,
    pub constructor: PersistentConstructorId,
    pub variant: PersistentEnumVariantId,
    pub function: PersistentFunctionId,
    pub getter: PersistentPropertyAccessorId,
    pub setter: PersistentPropertyAccessorId,
    pub slot: PersistentDispatchSlotId,
    pub object: PersistentObjectValueId,
}
impl Fixture {
    pub fn new() -> Self {
        let provider = ConeCoordinate::new("example", "selected-types", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let alternate = ConeCoordinate::new("example", "other-selected-types", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let (owner, owner_id, _) = nominal(provider, "Base", SourceNominalKind::Class);
        let (derived, _, _) = nominal(provider, "Derived", SourceNominalKind::Class);
        let (enumeration, _, enum_key) = nominal(provider, "Choice", SourceNominalKind::Enum);
        let (interface, _, _) = nominal(provider, "Readable", SourceNominalKind::Interface);
        let (object_exact, _, object_key) =
            nominal(provider, "Singleton", SourceNominalKind::Object);
        let constructor = PersistentConstructorId::from_source_declaration(
            &SourceDeclarationKey::constructor(site(provider, Some(owner_id)), vec![]),
        )
        .unwrap();
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                site(provider, Some(owner_id)),
                name("read"),
                0,
                None,
                vec![],
            ))
            .unwrap();
        let property = PersistentPropertyId::from_source_declaration(
            &SourceDeclarationKey::property(site(provider, Some(owner_id)), name("value")),
        )
        .unwrap();
        let accessor = |role| {
            PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
                PropertyOwner::Property(property),
                role,
            ))
            .unwrap()
        };
        Self {
            provider,
            alternate,
            owner,
            derived,
            enumeration,
            interface,
            object_exact,
            constructor,
            variant: PersistentEnumVariantId::from_key(
                &EnumVariantIdentityKey::source(&enum_key, name("Only")).unwrap(),
            )
            .unwrap(),
            function,
            getter: accessor(AccessorRole::Getter),
            setter: accessor(AccessorRole::Setter),
            slot: PersistentDispatchSlotId::from_key(&DispatchSlotKey::virtual_method(function))
                .unwrap(),
            object: PersistentObjectValueId::from_source_object(&object_key).unwrap(),
        }
    }
    pub fn record(&self, usage: SelectedTypeUseV1) -> SelectedExternalTypeUseV1 {
        SelectedExternalTypeUseV1::new(self.provider, usage)
    }
    pub fn signature(&self) -> SelectedExternalTypeUseV1 {
        self.record(SelectedTypeUseV1::Signature { exact: self.owner })
    }
    pub fn resolver(&self) -> Resolver<'_> {
        Resolver {
            fixture: self,
            calls: vec![],
        }
    }
}
fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
fn site(cone: ConeIdentity, owner: Option<PersistentTypeId>) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(
            owner.into_iter().map(DefinitionOwnerAtom::Type).collect(),
        ),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
fn nominal(
    cone: ConeIdentity,
    identifier: &str,
    kind: SourceNominalKind,
) -> (
    PersistentExactTypeId,
    PersistentTypeId,
    SourceDeclarationKey,
) {
    let key = SourceDeclarationKey::nominal(site(cone, None), name(identifier), kind, 0);
    let id = PersistentTypeId::from_source_declaration(&key).unwrap();
    (
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(id)).unwrap(),
        id,
        key,
    )
}
