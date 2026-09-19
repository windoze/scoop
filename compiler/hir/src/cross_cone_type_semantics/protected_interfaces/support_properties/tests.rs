use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};
use crate::*;
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, PersistentTypeId, PropertyOwner, SignatureTypeKey,
    SourceNominalKind,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

mod constants;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
impl NominalSupportPropertySemanticAuthority<&'static str> for Fixture {
    fn const_source(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&ConstPropertyDeclarationSourceV1, &'static str> {
        self.const_sources.get(&property).ok_or("unknown const")
    }
    fn canonical_const_value_type(
        &self,
        kind: CanonicalConstValueKindV1,
    ) -> Result<PersistentTypeId, &'static str> {
        self.const_types
            .get(&kind)
            .copied()
            .ok_or("unknown const type")
    }
}

#[test]
fn nested_public_property_preserves_its_protected_owner_and_real_setter_access() {
    for visibility in [DeclaredVisibilityV1::Public, DeclaredVisibilityV1::Private] {
        let mut fixture = Fixture::default();
        let outer = fixture.class("Outer");
        let owner = fixture
            .graph
            .add("Nested", SourceNominalKind::Class, &[outer]);
        fixture.graph.access.insert(
            owner.source,
            DeclarationAccessSourceV1::try_new(
                DeclaredVisibilityV1::Protected,
                vec![outer.source],
                fixture.graph.origins[&owner.source].clone(),
            )
            .unwrap(),
        );
        let value_type = SignatureTypeKey::Nominal(nominal(fixture.unit));
        let getter = fixture.accessor(owner, AccessorRole::Getter, value_type.clone());
        let setter = fixture.accessor(owner, AccessorRole::Setter, value_type.clone());
        let (CallableTemplateOrigin::Accessor(get), CallableTemplateOrigin::Accessor(set)) =
            (getter, setter)
        else {
            unreachable!()
        };
        let PropertyOwner::Property(property) = fixture.accessors[&get].owner() else {
            unreachable!()
        };
        fixture.property_shapes.insert(
            property,
            ProtectedPropertySourceShapeV1 {
                getter: get,
                setter: Some((set, visibility)),
                representation: PropertyRepresentationV1::RuntimeAccessor,
            },
        );
        let getter = NominalSupportCallableInterfaceV1::try_new(
            getter,
            fixture.access(owner, DeclaredVisibilityV1::Public),
            fixture
                .payload(owner, getter, vec![], value_type.clone())
                .source_signature,
        )
        .unwrap();
        let setter = NominalSupportCallableInterfaceV1::try_new(
            setter,
            fixture.access(owner, visibility),
            fixture
                .payload(owner, setter, vec![value_type.clone()], value_type.clone())
                .source_signature,
        )
        .unwrap();
        let source = NominalSourcePropertyPayloadV1::try_new(
            owner.source,
            value_type,
            get,
            ProtectedPropertyMutabilityV1::ReadWrite {
                setter: set,
                setter_access: fixture.access(owner, visibility),
            },
            PropertyRepresentationV1::RuntimeAccessor,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap();
        if visibility == DeclaredVisibilityV1::Public {
            assert!(ProtectedPropertyPayloadV1::from_source_payload(source.clone()).is_err());
        }
        let record = NominalSupportPropertyInterfaceV1::try_new(
            property,
            fixture.access(owner, DeclaredVisibilityV1::Public),
            NominalSupportPropertyPayloadV1::Runtime { interface: source },
        )
        .unwrap();
        let bytes = encode(&record).unwrap();
        let decoded: DecodedNominalSupportPropertyInterfaceV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), record);
        let graph_source = fixture.graph.clone();
        let graph = CheckedNominalInheritanceGraphV1::validate(
            graph_source.records.values(),
            &graph_source,
            &mut meter(),
        )
        .unwrap();
        let mut getter_authority = fixture.clone();
        let mut setter_authority = fixture.clone();
        let checked_getter = getter
            .validate_source(&graph, &mut getter_authority, &mut meter())
            .unwrap();
        let checked_setter = setter
            .validate_source(&graph, &mut setter_authority, &mut meter())
            .unwrap();
        let CheckedNominalSupportPropertySourceV1::Runtime(checked) = record
            .validate_source(&graph, &mut fixture, &mut meter())
            .unwrap()
        else {
            panic!("runtime property required")
        };
        checked
            .validate_accessor_contracts(checked_getter, Some(checked_setter), &mut meter())
            .unwrap();
        assert!(matches!(
            checked.validate_accessor_contracts(checked_getter, None, &mut meter()),
            Err(ProtectedPropertyAccessorClosureError::Setter)
        ));
        let access = graph
            .replay_declaration_access(checked.declaration_access(), &mut meter())
            .unwrap();
        assert!(!access.lookup().domain().is_universal());
    }
}
