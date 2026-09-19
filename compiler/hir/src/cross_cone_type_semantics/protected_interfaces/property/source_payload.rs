use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSourcePropertyPayloadV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) owner: SourceNominalId,
    pub(in crate::cross_cone_type_semantics::protected_interfaces) value_type: SignatureTypeKey,
    pub(in crate::cross_cone_type_semantics::protected_interfaces) getter:
        PersistentPropertyAccessorId,
    pub(in crate::cross_cone_type_semantics::protected_interfaces) mutability:
        ProtectedPropertyMutabilityV1,
    pub(in crate::cross_cone_type_semantics::protected_interfaces) representation:
        PropertyRepresentationV1,
    pub(in crate::cross_cone_type_semantics::protected_interfaces) slot_relations:
        CanonicalProtectedSlotRefsV1,
}
impl NominalSourcePropertyPayloadV1 {
    pub fn try_new(
        owner: SourceNominalId,
        value_type: SignatureTypeKey,
        getter: PersistentPropertyAccessorId,
        mutability: ProtectedPropertyMutabilityV1,
        representation: PropertyRepresentationV1,
        slot_relations: CanonicalProtectedSlotRefsV1,
    ) -> Result<Self, ProtectedPropertyBuildError> {
        use ProtectedPropertyBuildError as Error;
        if representation == PropertyRepresentationV1::Const {
            return Err(Error::Representation);
        }
        if representation == PropertyRepresentationV1::AbstractSlot && slot_relations.is_empty() {
            return Err(Error::MissingSlot);
        }
        if let ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } = &mutability
        {
            if *setter == getter {
                return Err(Error::Accessor);
            }
            if setter_access.lexical_owners().last().copied() != Some(owner) {
                return Err(Error::Owner);
            }
        }
        Ok(Self {
            owner,
            value_type,
            getter,
            mutability,
            representation,
            slot_relations,
        })
    }
    pub const fn owner(&self) -> SourceNominalId {
        self.owner
    }
    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
    pub const fn getter(&self) -> PersistentPropertyAccessorId {
        self.getter
    }
    pub const fn mutability(&self) -> &ProtectedPropertyMutabilityV1 {
        &self.mutability
    }
    pub const fn representation(&self) -> PropertyRepresentationV1 {
        self.representation
    }
    pub const fn slot_relations(&self) -> &CanonicalProtectedSlotRefsV1 {
        &self.slot_relations
    }
}
impl WireEncode for NominalSourcePropertyPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.getter.encode(encoder)?;
        encoder.field(4)?;
        self.mutability.encode(encoder)?;
        encoder.field(5)?;
        self.representation.encode(encoder)?;
        encoder.field(6)?;
        self.slot_relations.encode(encoder)
    }
}
