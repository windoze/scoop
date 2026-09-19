use super::*;

impl Fixture {
    pub fn protected_function(&mut self, owner: Node) -> PersistentFunctionId {
        let declaration = self.source.function(owner, "member", false, vec![]);
        let CallableTemplateOrigin::Function(id) = declaration else {
            unreachable!()
        };
        let payload = self.source.payload(
            owner,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(owner)),
        );
        let record = ProtectedDeclarationInterfaceV1::Callable(Box::new(self.source.record(
            owner,
            declaration,
            payload,
        )));
        self.member(owner, record);
        self.protocol(declaration);
        id
    }
    pub fn protected_constructor(&mut self, owner: Node) -> PersistentConstructorId {
        let id = self.source.constructor(owner);
        let declaration = CallableTemplateOrigin::Constructor(id);
        let payload = self.source.payload(
            owner,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(owner)),
        );
        let access = self.source.access(owner, DeclaredVisibilityV1::Protected);
        let source =
            NominalSupportConstructorInterfaceV1::try_new(id, access.clone(), (*payload).clone())
                .unwrap();
        self.source
            .inheritance_interfaces
            .constructor_sources
            .insert(id, source);
        self.source.inheritance_interfaces.constructors.insert(
            owner.exact,
            CanonicalPersistentIdsV1::try_new(vec![id]).unwrap(),
        );
        self.declarations
            .push(ProtectedDeclarationInterfaceV1::Constructor(Box::new(
                ProtectedConstructorInterfaceV1::try_new(id, access, payload).unwrap(),
            )));
        self.refresh();
        self.protocol(declaration);
        id
    }
    pub fn protected_getter(&mut self, owner: Node) -> PersistentPropertyAccessorId {
        let value = SignatureTypeKey::Nominal(nominal(owner));
        let declaration = self
            .source
            .accessor(owner, AccessorRole::Getter, value.clone());
        let CallableTemplateOrigin::Accessor(id) = declaration else {
            unreachable!()
        };
        let scoop_identity::PropertyOwner::Property(property) = self.source.accessors[&id].owner()
        else {
            unreachable!()
        };
        self.source.property_shapes.insert(
            property,
            ProtectedPropertySourceShapeV1 {
                getter: id,
                setter: None,
                representation: PropertyRepresentationV1::RuntimeAccessor,
            },
        );
        let getter = ProtectedDeclarationInterfaceV1::Callable(Box::new(
            self.source.record(
                owner,
                declaration,
                self.source
                    .payload(owner, declaration, vec![], value.clone()),
            ),
        ));
        let property = ProtectedDeclarationInterfaceV1::Property(Box::new(
            ProtectedPropertyInterfaceV1::try_new(
                property,
                self.source.access(owner, DeclaredVisibilityV1::Protected),
                ProtectedPropertyPayloadV1::try_new(
                    owner.source,
                    value,
                    id,
                    ProtectedPropertyMutabilityV1::ReadOnly,
                    PropertyRepresentationV1::RuntimeAccessor,
                    CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        ));
        self.member(owner, getter);
        self.member(owner, property);
        id
    }
    fn member(&mut self, owner: Node, record: ProtectedDeclarationInterfaceV1) {
        let mut members = self.source.inheritance_interfaces.members[&owner.exact]
            .values()
            .to_vec();
        members.push(record.reference());
        self.source.inheritance_interfaces.members.insert(
            owner.exact,
            CanonicalProtectedDeclarationRefsV1::try_new(members).unwrap(),
        );
        self.declarations.push(record);
        self.refresh();
    }
    fn refresh(&mut self) {
        self.source.protected_roots = CanonicalProtectedDeclarationRefsV1::try_new(
            self.declarations
                .iter()
                .map(ProtectedDeclarationInterfaceV1::reference)
                .collect(),
        )
        .unwrap();
    }
    fn protocol(&mut self, declaration: CallableTemplateOrigin) {
        self.protocols.push(
            ProtectedCallableSourceInterfaceV1::try_new(
                declaration,
                CanonicalProtectedSourceParametersV1::try_new(vec![]).unwrap(),
            )
            .unwrap(),
        );
    }
}
fn nominal(node: Node) -> PersistentTypeId {
    let SourceNominalId::Concrete(id) = node.source else {
        unreachable!()
    };
    id
}
