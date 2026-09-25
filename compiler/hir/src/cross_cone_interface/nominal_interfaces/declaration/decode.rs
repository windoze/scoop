use super::*;
use crate::{DecodedNestedSourceMemberRefV1, NestedSourceBuildError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalDeclarationDetailsV1 {
    modality: NominalInheritanceModalityV1,
    visibility: DeclaredVisibilityV1,
    constructors: DecodedCanonicalPersistentIdsV1<PersistentConstructorId>,
    members: Vec<DecodedNestedSourceMemberRefV1>,
    children: Vec<DecodedSourceNominalId>,
    dispatch_order: DecodedNominalDispatchOrderV1,
    dispatch_selections: DecodedCanonicalNominalDispatchSelectionsV1,
}

impl DecodedNominalDeclarationDetailsV1 {
    pub fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalDeclarationDetailsV1, NominalDeclarationDetailsResolutionError<E>> {
        use NominalDeclarationDetailsResolutionError as Error;
        let constructors = self
            .constructors
            .resolve(resolver)
            .map_err(Error::Constructors)?;
        let members = self
            .members
            .into_iter()
            .map(|member| member.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::Reference)?;
        let children = self
            .children
            .into_iter()
            .map(|child| child.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::Reference)?;
        let members = CanonicalNestedMemberRefsV1::from_ordered(members).map_err(Error::Order)?;
        let children =
            CanonicalNestedNominalRefsV1::from_ordered(children).map_err(Error::Order)?;
        Ok(NominalDeclarationDetailsV1::new(
            self.modality,
            self.visibility,
            constructors,
            members,
            children,
            self.dispatch_order
                .resolve(resolver)
                .map_err(Error::DispatchOrder)?,
            self.dispatch_selections
                .resolve(resolver)
                .map_err(Error::DispatchSelections)?,
        ))
    }
}

impl WireDecode for DecodedNominalDeclarationDetailsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        let value = Self {
            modality: decoder.field(1, NominalInheritanceModalityV1::decode)?,
            visibility: decoder.field(2, DeclaredVisibilityV1::decode)?,
            constructors: decoder.field(3, DecodedCanonicalPersistentIdsV1::decode)?,
            members: decoder.field(4, |d| {
                d.decode_array(|d, _| DecodedNestedSourceMemberRefV1::decode(d))
            })?,
            children: decoder.field(5, |d| {
                d.decode_array(|d, _| DecodedSourceNominalId::decode(d))
            })?,
            dispatch_order: decoder.field(6, DecodedNominalDispatchOrderV1::decode)?,
            dispatch_selections: decoder
                .field(7, DecodedCanonicalNominalDispatchSelectionsV1::decode)?,
        };

        Ok(value)
    }
}

impl WireEncode for DecodedNominalDeclarationDetailsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.modality.encode(encoder)?;
        encoder.field(2)?;
        self.visibility.encode(encoder)?;
        encoder.field(3)?;
        self.constructors.encode(encoder)?;
        encoder.field(4)?;
        encoder.array(self.members.len() as u64)?;
        for value in &self.members {
            value.encode(encoder)?;
        }
        encoder.field(5)?;
        encoder.array(self.children.len() as u64)?;
        for value in &self.children {
            value.encode(encoder)?;
        }
        encoder.field(6)?;
        self.dispatch_order.encode(encoder)?;
        encoder.field(7)?;
        self.dispatch_selections.encode(encoder)
    }
}

#[derive(Debug)]
pub enum NominalDeclarationDetailsResolutionError<E> {
    DispatchOrder(NominalDispatchOrderResolutionError<E>),
    DispatchSelections(NominalDispatchSelectionResolutionError<E>),
    Constructors(CanonicalPersistentIdSetValidationError<PersistentConstructorId, E>),
    Reference(E),
    Order(NestedSourceBuildError),
}

impl<E: std::fmt::Display> std::fmt::Display for NominalDeclarationDetailsResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DispatchOrder(e) => e.fmt(f),
            Self::DispatchSelections(e) => e.fmt(f),
            Self::Constructors(e) => e.fmt(f),
            Self::Reference(e) => e.fmt(f),
            Self::Order(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for NominalDeclarationDetailsResolutionError<E>
{
}
