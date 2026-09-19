use super::*;

impl Fixture {
    pub fn origins(&self) -> CanonicalExportDefinitionSourcesV1 {
        let mut origins = self
            .representations
            .values()
            .iter()
            .map(|id| self.source.graph.origins[&SourceNominalId::Concrete(*id)].clone())
            .collect::<Vec<_>>();
        for slot in self.slots.values().flatten() {
            for origin in std::iter::once(slot.declaration_access().definition_origin()).chain(
                slot.implementation()
                    .target()
                    .map(|target| target.declaration_access().definition_origin()),
            ) {
                if !origins.contains(origin) {
                    origins.push(origin.clone());
                }
            }
        }
        CanonicalExportDefinitionSourcesV1::try_new(origins).unwrap()
    }
    pub fn object(&mut self, name: &str) -> (Node, PersistentObjectValueId, PersistentExactTypeId) {
        let node = self.add_kind(name, SourceNominalKind::Object, false);
        let SourceNominalId::Concrete(owner) = node.source else {
            unreachable!()
        };
        let key = GeneratedNominalKey::ObjectBackingClass { object: owner };
        let backing = PersistentTypeId::from_generated_key(&key).unwrap();
        self.source.graph.generated.insert(backing, key);
        let key = ExactTypeKey::Nominal(backing);
        let exact = PersistentExactTypeId::from_key(&key).unwrap();
        self.source.graph.exacts.insert(exact, key);
        self.source.graph.representations.insert(
            owner,
            NominalRepresentationSupportV1::try_new(
                &self.source.graph.keys[&node.source],
                self.source.graph.access[&node.source].clone(),
                NominalRepresentationShapeV1::Object {
                    backing_class: backing,
                    declared_fields: vec![],
                },
            )
            .unwrap(),
        );
        self.edges
            .push(self.source.graph.records[&node.exact].clone());
        self.edges.sort_by_key(NominalInheritanceEdgesV1::owner);
        self.representations = CanonicalPersistentIdsV1::try_new(
            self.representations
                .values()
                .iter()
                .copied()
                .chain([owner])
                .collect(),
        )
        .unwrap();
        self.shapes
            .insert(node.exact, ExactTypeFactShapeV1::Reference);
        self.facts =
            CanonicalPersistentIdsV1::try_new(self.shapes.keys().copied().collect()).unwrap();
        let inventory = &mut self.source.inheritance_interfaces;
        inventory.owners = CanonicalPersistentIdsV1::try_new(
            self.edges
                .iter()
                .map(NominalInheritanceEdgesV1::owner)
                .collect(),
        )
        .unwrap();
        inventory
            .constructors
            .insert(node.exact, CanonicalPersistentIdsV1::empty());
        inventory
            .members
            .insert(node.exact, CanonicalProtectedDeclarationRefsV1::default());
        inventory.schemas.insert(
            node.exact,
            CanonicalInheritanceSlotSchemasV1::try_new(vec![
                InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, vec![])
                    .unwrap(),
            ])
            .unwrap(),
        );
        (
            node,
            PersistentObjectValueId::from_source_object(&self.source.graph.keys[&node.source])
                .unwrap(),
            exact,
        )
    }
    pub fn base(&mut self, derived: Node, base: Node) {
        self.source.graph.edges(derived, Some(base), &[]);
        *self
            .edges
            .iter_mut()
            .find(|record| record.owner() == derived.exact)
            .unwrap() = self.source.graph.records[&derived.exact].clone();
        let SourceNominalId::Concrete(owner) = derived.source else {
            unreachable!()
        };
        let SourceNominalId::Concrete(base) = base.source else {
            unreachable!()
        };
        self.source.graph.representations.insert(
            owner,
            NominalRepresentationSupportV1::try_new(
                &self.source.graph.keys[&derived.source],
                self.source.graph.access[&derived.source].clone(),
                NominalRepresentationShapeV1::Class {
                    base: OptionalSignatureType::Present(Box::new(SignatureTypeKey::Nominal(base))),
                    declared_fields: vec![],
                },
            )
            .unwrap(),
        );
    }
    pub fn virtual_slot(&mut self, owner: Node) -> PersistentDispatchSlotId {
        let callable = self.source.function(owner, "virtualMethod", false, vec![]);
        let CallableTemplateOrigin::Function(function) = callable else {
            unreachable!()
        };
        let declaration = InheritanceCallableDeclarationV1::Function(function);
        let SourceNominalId::Concrete(owner_id) = owner.source else {
            unreachable!()
        };
        let effects = self
            .source
            .payload(owner, callable, vec![], SignatureTypeKey::Nominal(owner_id))
            .effects();
        let signature = InheritanceCallableSignatureV1::try_new(
            ExactCallableSignature::new(
                scoop_identity::Effect::Ordinary,
                Some(owner.exact),
                vec![],
                owner.exact,
            ),
            effects,
        )
        .unwrap();
        let access = self.source.access(owner, DeclaredVisibilityV1::Public);
        let key = DispatchSlotKey::virtual_method(function);
        let slot = PersistentDispatchSlotId::from_key(&key).unwrap();
        self.source
            .inheritance_interfaces
            .slot_keys
            .insert(slot, key);
        self.source.slots.push(slot);
        self.source.inheritance_interfaces.callables.insert(
            declaration,
            (signature.clone(), CallableModalityV1::Open, access.clone()),
        );
        self.source.inheritance_interfaces.selections.insert(
            (owner.exact, slot),
            InheritanceSourceSlotSelectionV1::Concrete(declaration),
        );
        self.source.inheritance_interfaces.schemas.insert(
            owner.exact,
            CanonicalInheritanceSlotSchemasV1::try_new(vec![
                InheritanceSlotSchemaV1::try_new(
                    InheritanceSlotSchemaRoleV1::ClassVtable,
                    vec![slot],
                )
                .unwrap(),
            ])
            .unwrap(),
        );
        self.slots.insert(
            owner.exact,
            vec![
                InheritanceSlotContractV1::try_new(
                    slot,
                    owner_id,
                    declaration,
                    signature.clone(),
                    PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
                    InheritanceSlotImplementationV1::Concrete(
                        InheritanceSlotTargetV1::try_new(
                            declaration,
                            owner_id,
                            signature,
                            CallableModalityV1::Open,
                            access.clone(),
                        )
                        .unwrap(),
                    ),
                    access,
                )
                .unwrap(),
            ],
        );
        slot
    }
    pub fn inherit_slot(
        &mut self,
        derived: Node,
        provider: &Fixture,
        base: Node,
        slot: PersistentDispatchSlotId,
    ) {
        self.base(derived, base);
        self.source
            .declarations
            .extend(provider.source.declarations.clone());
        self.source.slots.push(slot);
        self.source
            .inheritance_interfaces
            .slot_keys
            .extend(provider.source.inheritance_interfaces.slot_keys.clone());
        self.source
            .inheritance_interfaces
            .callables
            .extend(provider.source.inheritance_interfaces.callables.clone());
        let contract = provider.slots[&base.exact][0].clone();
        self.source.inheritance_interfaces.selections.insert(
            (derived.exact, slot),
            InheritanceSourceSlotSelectionV1::Concrete(contract.declaration()),
        );
        self.source.inheritance_interfaces.schemas.insert(
            derived.exact,
            provider.source.inheritance_interfaces.schemas[&base.exact].clone(),
        );
        self.slots.insert(derived.exact, vec![contract]);
    }
}
