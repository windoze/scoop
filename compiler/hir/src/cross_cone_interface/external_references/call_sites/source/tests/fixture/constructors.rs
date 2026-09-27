use super::*;
use scoop_identity::PersistentConstructorId;

impl Fixture {
    pub fn into_constructor(mut self) -> Self {
        let source = self
            .public
            .callable_interfaces()
            .all_declarations()
            .next()
            .unwrap();
        let owner = match source.owner().nominal_owner().unwrap() {
            SourceNominalId::Concrete(owner) => DefinitionOwnerAtom::Type(owner),
            SourceNominalId::GenericTemplate(owner) => DefinitionOwnerAtom::GenericType(owner),
        };
        let identity = CborIdentityRecord::<PersistentConstructorId, _>::from_key(
            SourceDeclarationKey::constructor(
                site(vec![owner]),
                source
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.value_type().clone())
                    .collect(),
            ),
        )
        .unwrap();
        let target = CallableTemplateOrigin::Constructor(identity.id());
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(&self.identities)
            .unwrap();
        pending
            .register_external_canonical_authority(identity)
            .unwrap();
        self.identities = pending.finish().unwrap();
        let declaration = crate::CallableDeclarationRecordV1::try_new(
            target,
            source.owner(),
            source.type_parameters().clone(),
            None,
            source.parameters().clone(),
            source.result().clone(),
            source.effects(),
            source.modality(),
            source.declared_visibility(),
            source.slot_relations().clone(),
        )
        .unwrap();
        self.public = crate::CrossConeHirInterfaceSectionV1::new(
            Default::default(),
            Default::default(),
            crate::CanonicalCallableInterfacesV1::with_support(Vec::new(), vec![declaration])
                .unwrap(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        self.target = ExternalHirTargetV1::Callable(target);
        self.receiver = crate::SourceCallReceiver::NoReceiver;
        self
    }
}
