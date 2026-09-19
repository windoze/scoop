use super::*;
use scoop_identity::{DispatchSlotKey, ExactCallableSignature, SignatureTypeKey};

pub(in crate::cross_cone_type_semantics) struct Bundle {
    pub fixture: Fixture,
    pub table: CanonicalNominalInheritanceInterfacesV1,
    pub protected: CanonicalProtectedDeclarationInterfacesV1,
    pub base: Node,
    pub derived: Node,
    pub root: InheritanceCallableDeclarationV1,
    pub target: InheritanceCallableDeclarationV1,
    pub slot: PersistentDispatchSlotId,
}
impl Bundle {
    pub fn validate(&mut self) -> Result<(), InheritanceInterfaceSemanticError<&'static str>> {
        let graph_source = self.fixture.graph.clone();
        let graph = CheckedNominalInheritanceGraphV1::validate(
            graph_source.records.values(),
            &graph_source,
            &mut meter(),
        )
        .unwrap();
        let protected = self
            .protected
            .validate_sources(
                &graph,
                &CanonicalNominalRepresentationSupportV1::try_new(vec![]).unwrap(),
                &mut self.fixture,
                &mut meter(),
            )
            .unwrap();
        self.table
            .validate_interfaces(&graph, protected, &mut self.fixture, &mut meter())
            .map(|_| ())
    }
    pub fn change(&mut self, owner: Node, change: impl FnOnce(&mut NominalInheritanceInterfaceV1)) {
        let mut records = self.table.records().to_vec();
        change(
            records
                .iter_mut()
                .find(|record| record.owner() == owner.exact)
                .unwrap(),
        );
        self.table = CanonicalNominalInheritanceInterfacesV1::try_new(records).unwrap();
    }
}

pub(in crate::cross_cone_type_semantics) fn fixture() -> Bundle {
    let mut fixture = Fixture::default();
    let base = fixture.class("Base");
    let derived = fixture.class("Derived");
    fixture.graph.edges(derived, Some(base), &[]);
    let (root, base_signature) = callable(&mut fixture, base, CallableModalityV1::Open);
    let (target, derived_signature) = callable(&mut fixture, derived, CallableModalityV1::Final);
    let InheritanceCallableDeclarationV1::Function(root_function) = root else {
        unreachable!()
    };
    let slot_key = DispatchSlotKey::virtual_method(root_function);
    let slot = PersistentDispatchSlotId::from_key(&slot_key).unwrap();
    fixture.slots.push(slot);
    fixture
        .inheritance_interfaces
        .slot_keys
        .insert(slot, slot_key);
    let base_constructor = constructor(&mut fixture, base, DeclaredVisibilityV1::Protected);
    let derived_constructor = constructor(&mut fixture, derived, DeclaredVisibilityV1::Public);
    let helper_id = fixture.function(base, "helper", false, vec![]);
    let helper = ProtectedDeclarationInterfaceV1::Callable(Box::new(fixture.record(
        base,
        helper_id,
        fixture.payload(
            base,
            helper_id,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        ),
    )));
    let ctor_id = base_constructor.declaration();
    let protected_ctor = ProtectedDeclarationInterfaceV1::Constructor(Box::new(
        ProtectedConstructorInterfaceV1::try_new(
            ctor_id,
            fixture.access(base, DeclaredVisibilityV1::Protected),
            fixture.payload(
                base,
                CallableTemplateOrigin::Constructor(ctor_id),
                vec![],
                SignatureTypeKey::Nominal(nominal(base)),
            ),
        )
        .unwrap(),
    ));
    fixture.protected_roots = CanonicalProtectedDeclarationRefsV1::try_new(vec![
        helper.reference(),
        protected_ctor.reference(),
    ])
    .unwrap();
    let protected =
        CanonicalProtectedDeclarationInterfacesV1::try_new(vec![helper.clone(), protected_ctor])
            .unwrap();
    let schema = CanonicalInheritanceSlotSchemasV1::try_new(vec![
        InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, vec![slot])
            .unwrap(),
    ])
    .unwrap();
    let mut records = Vec::new();
    for (owner, declaration, signature, modality, constructor, members) in [
        (
            base,
            root,
            base_signature.clone(),
            CallableModalityV1::Open,
            base_constructor,
            vec![helper.reference()],
        ),
        (
            derived,
            target,
            derived_signature,
            CallableModalityV1::Final,
            derived_constructor,
            vec![],
        ),
    ] {
        let implementation = InheritanceSlotImplementationV1::Concrete(
            InheritanceSlotTargetV1::try_new(
                declaration,
                nominal(owner),
                signature,
                modality,
                fixture.access(owner, DeclaredVisibilityV1::Public),
            )
            .unwrap(),
        );
        let slot_contract = InheritanceSlotContractV1::try_new(
            slot,
            nominal(base),
            root,
            base_signature.clone(),
            PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
            implementation,
            fixture.access(base, DeclaredVisibilityV1::Public),
        )
        .unwrap();
        let members = CanonicalProtectedDeclarationRefsV1::try_new(members).unwrap();
        fixture.inheritance_interfaces.constructors.insert(
            owner.exact,
            CanonicalPersistentIdsV1::try_new(vec![constructor.declaration()]).unwrap(),
        );
        fixture
            .inheritance_interfaces
            .members
            .insert(owner.exact, members.clone());
        fixture
            .inheritance_interfaces
            .schemas
            .insert(owner.exact, schema.clone());
        fixture.inheritance_interfaces.selections.insert(
            (owner.exact, slot),
            InheritanceSourceSlotSelectionV1::Concrete(declaration),
        );
        let graph = CheckedNominalInheritanceGraphV1::validate(
            fixture.graph.records.values(),
            &fixture.graph,
            &mut meter(),
        )
        .unwrap();
        let domains = graph
            .replay_nominal_domains(owner.exact, &mut meter())
            .unwrap()
            .to_record();
        records.push(
            NominalInheritanceInterfaceV1::try_new(
                fixture.graph.records[&owner.exact].clone(),
                domains,
                CanonicalInheritanceConstructorsV1::try_new(vec![constructor]).unwrap(),
                CanonicalInheritanceSlotContractsV1::try_new(vec![slot_contract]).unwrap(),
                members,
                schema.clone(),
            )
            .unwrap(),
        );
    }
    fixture.inheritance_interfaces.owners =
        CanonicalPersistentIdsV1::try_new(vec![base.exact, derived.exact]).unwrap();
    Bundle {
        fixture,
        table: CanonicalNominalInheritanceInterfacesV1::try_new(records).unwrap(),
        protected,
        base,
        derived,
        root,
        target,
        slot,
    }
}

pub(super) fn constructor(
    fixture: &mut Fixture,
    owner: Node,
    visibility: DeclaredVisibilityV1,
) -> InheritanceConstructorInterfaceV1 {
    let id = fixture.constructor(owner);
    let payload = fixture.payload(
        owner,
        CallableTemplateOrigin::Constructor(id),
        vec![],
        SignatureTypeKey::Nominal(nominal(owner)),
    );
    let source = NominalSupportConstructorInterfaceV1::try_new(
        id,
        fixture.access(owner, visibility),
        (*payload).clone(),
    )
    .unwrap();
    fixture
        .inheritance_interfaces
        .constructor_sources
        .insert(id, source.clone());
    InheritanceConstructorInterfaceV1::try_new(source).unwrap()
}
fn callable(
    fixture: &mut Fixture,
    owner: Node,
    modality: CallableModalityV1,
) -> (
    InheritanceCallableDeclarationV1,
    InheritanceCallableSignatureV1,
) {
    let declaration = fixture.function(owner, "method", false, vec![]);
    let CallableTemplateOrigin::Function(id) = declaration else {
        unreachable!()
    };
    let payload = fixture.payload(
        owner,
        declaration,
        vec![],
        SignatureTypeKey::Nominal(nominal(fixture.unit)),
    );
    let signature = InheritanceCallableSignatureV1::try_new(
        ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            Some(owner.exact),
            vec![],
            fixture.unit.exact,
        ),
        payload.effects(),
    )
    .unwrap();
    let declaration = InheritanceCallableDeclarationV1::Function(id);
    fixture.inheritance_interfaces.callables.insert(
        declaration,
        (
            signature.clone(),
            modality,
            fixture.access(owner, DeclaredVisibilityV1::Public),
        ),
    );
    (declaration, signature)
}
