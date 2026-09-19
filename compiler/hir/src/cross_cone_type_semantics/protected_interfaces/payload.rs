use super::{
    CanonicalProtectedSlotRefsV1, ProtectedCallableInterfaceBuildError as Error,
    ProtectedSourceInterfaceUseV1,
};
use crate::{
    CallableImplementationV1, CallableModalityV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalSourceParameterShapesV1, SourceNominalId,
};
use scoop_identity::{CallableTemplateOrigin, OptionalSignatureType, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode};

/// Source member metadata. Its absent extension receiver does not erase the
/// implicit nominal receiver, which is supplied by `owner` during exact lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedCallablePayloadV1 {
    pub(super) owner: SourceNominalId,
    pub(super) type_parameters: CanonicalBinderListV1,
    pub(super) parameters: CanonicalSourceParameterShapesV1,
    pub(super) result: SignatureTypeKey,
    pub(super) effects: CallableSourceEffectsV1,
    pub(super) modality: CallableModalityV1,
    pub(super) source_interface: ProtectedSourceInterfaceUseV1,
    pub(super) slot_relations: CanonicalProtectedSlotRefsV1,
}
impl ProtectedCallablePayloadV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: CallableTemplateOrigin,
        owner: SourceNominalId,
        type_parameters: CanonicalBinderListV1,
        parameters: CanonicalSourceParameterShapesV1,
        result: SignatureTypeKey,
        effects: CallableSourceEffectsV1,
        modality: CallableModalityV1,
        slot_relations: CanonicalProtectedSlotRefsV1,
    ) -> Result<Self, Error> {
        let source_interface = ProtectedSourceInterfaceUseV1::for_declaration(declaration)?;
        let value = Self {
            owner,
            type_parameters,
            parameters,
            result,
            effects,
            modality,
            source_interface,
            slot_relations,
        };
        value.validate_declaration(declaration)?;
        Ok(value)
    }
    pub(super) fn validate_declaration(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<(), Error> {
        if self.source_interface != ProtectedSourceInterfaceUseV1::for_declaration(declaration)? {
            return Err(Error::SourceInterface);
        }
        let generic = matches!(declaration, CallableTemplateOrigin::GenericFunction(_));
        if generic == self.type_parameters.is_empty() {
            return Err(Error::Binders);
        }
        if self.modality == CallableModalityV1::InterfaceDefault
            || matches!(
                self.effects.implementation(),
                CallableImplementationV1::SourceExternScoop
                    | CallableImplementationV1::SourceExternC
            )
        {
            return Err(Error::Modality);
        }
        if matches!(declaration, CallableTemplateOrigin::Constructor(_)) || generic {
            if self.modality != CallableModalityV1::Final || !self.slot_relations.is_empty() {
                return Err(Error::Modality);
            }
        } else if matches!(
            self.modality,
            CallableModalityV1::Open | CallableModalityV1::Abstract
        ) && self.slot_relations.is_empty()
        {
            return Err(Error::MissingSlot);
        }
        if matches!(
            declaration,
            CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::Accessor(_)
        ) && self.effects.execution() != scoop_identity::Effect::Ordinary
        {
            return Err(Error::Execution);
        }
        Ok(())
    }
    pub const fn owner(&self) -> SourceNominalId {
        self.owner
    }
    pub const fn type_parameters(&self) -> &CanonicalBinderListV1 {
        &self.type_parameters
    }
    pub const fn receiver(&self) -> OptionalSignatureType {
        OptionalSignatureType::Absent
    }
    pub const fn parameters(&self) -> &CanonicalSourceParameterShapesV1 {
        &self.parameters
    }
    pub const fn result(&self) -> &SignatureTypeKey {
        &self.result
    }
    pub const fn effects(&self) -> CallableSourceEffectsV1 {
        self.effects
    }
    pub const fn modality(&self) -> CallableModalityV1 {
        self.modality
    }
    pub const fn source_interface(&self) -> ProtectedSourceInterfaceUseV1 {
        self.source_interface
    }
    pub const fn slot_relations(&self) -> &CanonicalProtectedSlotRefsV1 {
        &self.slot_relations
    }
}
impl WireEncode for ProtectedCallablePayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(3)?;
        OptionalSignatureType::Absent.encode(encoder)?;
        encoder.field(4)?;
        self.parameters.encode(encoder)?;
        encoder.field(5)?;
        self.result.encode(encoder)?;
        encoder.field(6)?;
        self.effects.encode(encoder)?;
        encoder.field(7)?;
        self.modality.encode(encoder)?;
        encoder.field(8)?;
        self.source_interface.encode(encoder)?;
        encoder.field(9)?;
        self.slot_relations.encode(encoder)
    }
}
