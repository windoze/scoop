use super::*;
use std::sync::Arc;

impl NominalInheritanceSemanticAuthority<&'static str> for Fixture {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Result<&ExactTypeKey, &'static str> {
        self.graph.exact_type_key(id)
    }
    fn nominal_declaration_key(
        &self,
        id: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.graph.keys.get(&id).ok_or("unknown source")
    }
    fn nominal_access_source(
        &self,
        id: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, &'static str> {
        self.graph.nominal_access_source(id)
    }
    fn nominal_definition_source(
        &self,
        id: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        self.graph.origins.get(&id).ok_or("unknown origin")
    }
    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        self.graph.validate_definition_source(source)
    }
    fn object_representation(
        &self,
        id: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, &'static str> {
        self.graph.object_representation(id)
    }
    fn generated_nominal_key(
        &self,
        id: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, &'static str> {
        self.graph.generated_nominal_key(id)
    }
}
impl NominalInterfaceShapeAuthority<&'static str> for Fixture {
    fn concrete_nominal_shape(
        &mut self,
        id: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        let key = self
            .graph
            .keys
            .get(&SourceNominalId::Concrete(id))
            .ok_or("unknown nominal")?;
        Ok(PublicNominalShapeV1::new(
            PublicNominalKindV1::try_from(key.declaration_kind()).unwrap(),
            0,
        ))
    }
    fn generic_nominal_shape(
        &mut self,
        id: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        let key = self
            .graph
            .keys
            .get(&SourceNominalId::GenericTemplate(id))
            .ok_or("unknown generic nominal")?;
        Ok(PublicNominalShapeV1::new(
            PublicNominalKindV1::try_from(key.declaration_kind()).unwrap(),
            key.duplicate_signature().type_parameter_count(),
        ))
    }
}
impl ProtectedCallableSemanticAuthority<&'static str> for Fixture {
    fn callable_source_key(
        &self,
        id: CallableTemplateOrigin,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.declarations.get(&id).ok_or("unknown callable")
    }
    fn property_accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, &'static str> {
        self.accessors.get(&id).ok_or("unknown accessor")
    }
    fn property_value_type(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&SignatureTypeKey, &'static str> {
        self.property_types.get(&id).ok_or("unknown property")
    }
    fn unit_type(&self) -> Result<PersistentTypeId, &'static str> {
        Ok(nominal(self.unit))
    }
}

macro_rules! callable_resolver {
    ($id:ty, $variant:ident) => {
        impl PersistentIdResolver<$id> for Fixture {
            type Error = &'static str;
            fn resolve(&mut self, id: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                self.declarations
                    .keys()
                    .find_map(|known| match known {
                        CallableTemplateOrigin::$variant(known)
                            if known.as_array() == id.as_array() =>
                        {
                            Some(*known)
                        }
                        _ => None,
                    })
                    .ok_or("unknown callable")
            }
        }
    };
}
callable_resolver!(PersistentFunctionId, Function);
callable_resolver!(PersistentGenericFunctionId, GenericFunction);
callable_resolver!(PersistentConstructorId, Constructor);
callable_resolver!(PersistentPropertyAccessorId, Accessor);
impl PersistentIdResolver<PersistentEnumVariantId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentEnumVariantId>,
    ) -> Result<PersistentEnumVariantId, Self::Error> {
        self.variants
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown source variant")
    }
}
impl PersistentIdResolver<PersistentTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.graph
            .keys
            .keys()
            .find_map(|known| match known {
                SourceNominalId::Concrete(known) if known.as_array() == id.as_array() => {
                    Some(*known)
                }
                _ => None,
            })
            .ok_or("unknown nominal")
    }
}
impl PersistentIdResolver<PersistentGenericTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        self.graph
            .keys
            .keys()
            .find_map(|known| match known {
                SourceNominalId::GenericTemplate(known) if known.as_array() == id.as_array() => {
                    Some(*known)
                }
                _ => None,
            })
            .ok_or("unknown generic nominal")
    }
}
impl PersistentIdResolver<PersistentDispatchSlotId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentDispatchSlotId>,
    ) -> Result<PersistentDispatchSlotId, Self::Error> {
        self.slots
            .iter()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown slot")
    }
}
impl PersistentIdResolver<ConeIdentity> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE).map_err(|_| "unknown cone")
    }
}
impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Fixture {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        let context = SourceContextKey::File {
            source: self.graph.origins[&self.unit.source]
                .origin()
                .source()
                .clone(),
        };
        id.verify(PersistentSourceContextId::from_key(&context).unwrap())
            .map_err(|_| "unknown context")?;
        Ok(Arc::new(context))
    }
}
