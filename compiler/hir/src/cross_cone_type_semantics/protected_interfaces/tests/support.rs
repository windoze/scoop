use crate::cross_cone_type_semantics::inheritance::tests::support::{
    Fixture as GraphFixture, Node, site,
};
use crate::*;
use scoop_identity::*;
use scoop_identity::{GcEffect, PropertyOwner};
use std::collections::BTreeMap;

mod authority;

#[derive(Clone)]
pub(in crate::cross_cone_type_semantics) struct Fixture {
    pub graph: GraphFixture,
    pub unit: Node,
    pub declarations: BTreeMap<CallableTemplateOrigin, SourceDeclarationKey>,
    pub accessors: BTreeMap<PersistentPropertyAccessorId, PropertyAccessorKey>,
    pub property_types: BTreeMap<PersistentPropertyId, SignatureTypeKey>,
    pub slots: Vec<PersistentDispatchSlotId>,
    pub property_shapes: BTreeMap<PersistentPropertyId, ProtectedPropertySourceShapeV1>,
    pub variants: BTreeMap<PersistentEnumVariantId, (EnumVariantIdentityKey, EnumSourceVariantV1)>,
    pub variant_fields: BTreeMap<PersistentEnumVariantFieldId, EnumVariantFieldKey>,
    pub const_sources: BTreeMap<PersistentPropertyId, ConstPropertyDeclarationSourceV1>,
    pub const_types: BTreeMap<CanonicalConstValueKindV1, PersistentTypeId>,
    pub nominal_sources: BTreeMap<SourceNominalId, ProtectedNestedSourceInterfaceV1>,
    pub struct_fields: BTreeMap<PersistentFieldId, FieldIdentityKey>,
    pub protected_roots: CanonicalProtectedDeclarationRefsV1,
}
impl Default for Fixture {
    fn default() -> Self {
        let mut graph = GraphFixture::default();
        let unit = graph.add("Unit", SourceNominalKind::Struct, &[]);
        Self {
            graph,
            unit,
            declarations: BTreeMap::new(),
            accessors: BTreeMap::new(),
            property_types: BTreeMap::new(),
            slots: Vec::new(),
            property_shapes: BTreeMap::new(),
            variants: BTreeMap::new(),
            variant_fields: BTreeMap::new(),
            const_sources: BTreeMap::new(),
            const_types: BTreeMap::new(),
            nominal_sources: BTreeMap::new(),
            struct_fields: BTreeMap::new(),
            protected_roots: CanonicalProtectedDeclarationRefsV1::default(),
        }
    }
}
pub(in crate::cross_cone_type_semantics) fn nominal(node: Node) -> PersistentTypeId {
    let SourceNominalId::Concrete(id) = node.source else {
        unreachable!()
    };
    id
}
impl Fixture {
    pub fn owner_chain(&self, owner: Node) -> Vec<SourceNominalId> {
        let mut owners = self.graph.access[&owner.source].lexical_owners().to_vec();
        owners.push(owner.source);
        owners
    }
    pub fn class(&mut self, name: &str) -> Node {
        self.graph.add(name, SourceNominalKind::Class, &[])
    }
    pub fn function(
        &mut self,
        owner: Node,
        name: &str,
        generic: bool,
        parameters: Vec<SignatureTypeKey>,
    ) -> CallableTemplateOrigin {
        let key = SourceDeclarationKey::function(
            site(&self.owner_chain(owner)),
            CanonicalIdentifier::new(name).unwrap(),
            u32::from(generic),
            None,
            parameters,
        );
        let id = if generic {
            CallableTemplateOrigin::GenericFunction(
                PersistentGenericFunctionId::from_source_declaration(&key).unwrap(),
            )
        } else {
            CallableTemplateOrigin::Function(
                PersistentFunctionId::from_source_declaration(&key).unwrap(),
            )
        };
        self.declarations.insert(id, key);
        id
    }
    pub fn constructor(&mut self, owner: Node) -> PersistentConstructorId {
        let key = SourceDeclarationKey::constructor(site(&self.owner_chain(owner)), vec![]);
        let id = PersistentConstructorId::from_source_declaration(&key).unwrap();
        self.declarations
            .insert(CallableTemplateOrigin::Constructor(id), key);
        id
    }
    pub fn accessor(
        &mut self,
        owner: Node,
        role: AccessorRole,
        value: SignatureTypeKey,
    ) -> CallableTemplateOrigin {
        let key = SourceDeclarationKey::property(
            site(&self.owner_chain(owner)),
            CanonicalIdentifier::new("property").unwrap(),
        );
        let property = PersistentPropertyId::from_source_declaration(&key).unwrap();
        self.property_types.insert(property, value);
        let accessor = PropertyAccessorKey::new(PropertyOwner::Property(property), role);
        let id = PersistentPropertyAccessorId::from_key(&accessor).unwrap();
        self.accessors.insert(id, accessor);
        let declaration = CallableTemplateOrigin::Accessor(id);
        self.declarations.insert(declaration, key);
        declaration
    }
    pub fn access(
        &self,
        owner: Node,
        visibility: DeclaredVisibilityV1,
    ) -> DeclarationAccessSourceV1 {
        DeclarationAccessSourceV1::try_new(
            visibility,
            self.owner_chain(owner),
            self.graph.origins[&owner.source].clone(),
        )
        .unwrap()
    }
    pub fn payload(
        &self,
        owner: Node,
        declaration: CallableTemplateOrigin,
        parameter_types: Vec<SignatureTypeKey>,
        result: SignatureTypeKey,
    ) -> ProtectedCallablePayloadV1 {
        let binders = if matches!(declaration, CallableTemplateOrigin::GenericFunction(_)) {
            vec![TypeParameterBinderV1::new(
                CanonicalIdentifier::new("T").unwrap(),
                TypeParameterBoundsV1::Unconstrained,
            )]
        } else {
            vec![]
        };
        ProtectedCallablePayloadV1::try_new(
            declaration,
            owner.source,
            CanonicalBinderListV1::try_new(binders).unwrap(),
            CanonicalSourceParameterShapesV1::try_new(
                parameter_types
                    .into_iter()
                    .enumerate()
                    .map(|(index, ty)| {
                        SourceParameterShapeV1::new(
                            CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                            ty,
                        )
                    })
                    .collect(),
            )
            .unwrap(),
            result,
            CallableSourceEffectsV1::try_new(
                Effect::Ordinary,
                CallableSafetyV1::Safe,
                GcEffect::Managed,
                CallableImplementationV1::Scoop,
                CallableOperatorRoleV1::None,
                CallableInfixV1::Ordinary,
            )
            .unwrap(),
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap()
    }
    pub fn record(
        &self,
        owner: Node,
        declaration: CallableTemplateOrigin,
        payload: ProtectedCallablePayloadV1,
    ) -> ProtectedCallableInterfaceV1 {
        ProtectedCallableInterfaceV1::try_new(
            declaration,
            self.access(owner, DeclaredVisibilityV1::Protected),
            payload,
        )
        .unwrap()
    }
    pub fn slot(&mut self, declaration: CallableTemplateOrigin) -> PersistentDispatchSlotId {
        let CallableTemplateOrigin::Function(id) = declaration else {
            unreachable!()
        };
        let slot =
            PersistentDispatchSlotId::from_key(&DispatchSlotKey::virtual_method(id)).unwrap();
        self.slots.push(slot);
        slot
    }
}
