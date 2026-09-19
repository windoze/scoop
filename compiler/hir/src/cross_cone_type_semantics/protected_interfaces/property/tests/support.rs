use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::Node;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};

pub(in crate::cross_cone_type_semantics::protected_interfaces) fn setup(
    setter_visibility: Option<DeclaredVisibilityV1>,
) -> (
    Fixture,
    Node,
    ProtectedPropertyInterfaceV1,
    ProtectedCallableInterfaceV1,
    Option<ProtectedCallableInterfaceV1>,
) {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let value = SignatureTypeKey::Nominal(nominal(owner));
    let get = fixture.accessor(owner, AccessorRole::Getter, value.clone());
    let CallableTemplateOrigin::Accessor(getter) = get else {
        unreachable!()
    };
    let PropertyOwner::Property(property) = fixture.accessors[&getter].owner() else {
        unreachable!()
    };
    let getter_record = fixture.record(
        owner,
        get,
        fixture.payload(owner, get, vec![], value.clone()),
    );
    let mut setter_record = None;
    let mut setter_shape = None;
    let mutability = match setter_visibility {
        None => ProtectedPropertyMutabilityV1::ReadOnly,
        Some(visibility) => {
            let set = fixture.accessor(owner, AccessorRole::Setter, value.clone());
            let CallableTemplateOrigin::Accessor(setter) = set else {
                unreachable!()
            };
            if visibility == DeclaredVisibilityV1::Protected {
                setter_record = Some(fixture.record(
                    owner,
                    set,
                    fixture.payload(
                        owner,
                        set,
                        vec![value.clone()],
                        SignatureTypeKey::Nominal(nominal(fixture.unit)),
                    ),
                ));
            }
            setter_shape = Some((setter, visibility));
            ProtectedPropertyMutabilityV1::ReadWrite {
                setter,
                setter_access: fixture.access(owner, visibility),
            }
        }
    };
    fixture.property_shapes.insert(
        property,
        ProtectedPropertySourceShapeV1 {
            getter,
            setter: setter_shape,
            representation: PropertyRepresentationV1::RuntimeAccessor,
        },
    );
    let record = ProtectedPropertyInterfaceV1::try_new(
        property,
        fixture.access(owner, DeclaredVisibilityV1::Protected),
        ProtectedPropertyPayloadV1::try_new(
            owner.source,
            value,
            getter,
            mutability,
            PropertyRepresentationV1::RuntimeAccessor,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    (fixture, owner, record, getter_record, setter_record)
}

impl ProtectedPropertySemanticAuthority<&'static str> for Fixture {
    fn property_source_key(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        if let Some(source) = self.const_sources.get(&property) {
            return Ok(source.declaration());
        }
        self.declarations
            .values()
            .find(|key| PersistentPropertyId::from_source_declaration(key).ok() == Some(property))
            .ok_or("unknown property key")
    }
    fn property_source_shape(
        &self,
        property: PersistentPropertyId,
    ) -> Result<ProtectedPropertySourceShapeV1, &'static str> {
        self.property_shapes
            .get(&property)
            .copied()
            .ok_or("unknown property shape")
    }
}
impl PersistentIdResolver<PersistentPropertyId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentPropertyId>,
    ) -> Result<PersistentPropertyId, Self::Error> {
        self.property_types
            .keys()
            .copied()
            .find(|id| id.as_array() == decoded.as_array())
            .ok_or("unknown property id")
    }
}
