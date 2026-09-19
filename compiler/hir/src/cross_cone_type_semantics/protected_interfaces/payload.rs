use super::{
    CanonicalProtectedSlotRefsV1, NominalSourceCallablePayloadV1,
    NominalSupportSourceInterfaceUseV1, ProtectedCallableInterfaceBuildError,
    ProtectedSourceInterfaceUseV1,
};
use crate::{
    CallableModalityV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalSourceParameterShapesV1, SourceNominalId,
};
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode};

/// A class-only protected signature shape. The shared source constituent does
/// not grant visibility or imply that its owner is an ordinary public target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedCallablePayloadV1 {
    pub(super) source_signature: NominalSourceCallablePayloadV1,
    source_interface: ProtectedSourceInterfaceUseV1,
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
    ) -> Result<Self, ProtectedCallableInterfaceBuildError> {
        Self::from_source_signature(NominalSourceCallablePayloadV1::try_new(
            declaration,
            owner,
            type_parameters,
            parameters,
            result,
            effects,
            modality,
            slot_relations,
        )?)
    }
    pub(super) fn from_source_signature(
        source_signature: NominalSourceCallablePayloadV1,
    ) -> Result<Self, ProtectedCallableInterfaceBuildError> {
        if source_signature.modality() == CallableModalityV1::InterfaceDefault {
            return Err(ProtectedCallableInterfaceBuildError::Modality);
        }
        let source_interface = match source_signature.source_interface() {
            NominalSupportSourceInterfaceUseV1::AccessorNoSourceInterface => {
                ProtectedSourceInterfaceUseV1::AccessorNoSourceInterface
            }
            NominalSupportSourceInterfaceUseV1::Function(id) => {
                ProtectedSourceInterfaceUseV1::Function(id)
            }
            NominalSupportSourceInterfaceUseV1::GenericFunction(id) => {
                ProtectedSourceInterfaceUseV1::GenericFunction(id)
            }
            NominalSupportSourceInterfaceUseV1::Constructor(id) => {
                ProtectedSourceInterfaceUseV1::Constructor(id)
            }
            NominalSupportSourceInterfaceUseV1::VariantConstructor(_) => {
                return Err(ProtectedCallableInterfaceBuildError::DeclarationKind);
            }
        };
        Ok(Self {
            source_signature,
            source_interface,
        })
    }
    pub(super) fn validate_declaration(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<(), ProtectedCallableInterfaceBuildError> {
        self.source_signature.validate_declaration(declaration)?;
        if self.source_interface != ProtectedSourceInterfaceUseV1::for_declaration(declaration)? {
            return Err(ProtectedCallableInterfaceBuildError::SourceInterface);
        }
        if self.modality() == CallableModalityV1::InterfaceDefault {
            return Err(ProtectedCallableInterfaceBuildError::Modality);
        }
        Ok(())
    }
    pub const fn source_interface(&self) -> ProtectedSourceInterfaceUseV1 {
        self.source_interface
    }
}
impl std::ops::Deref for ProtectedCallablePayloadV1 {
    type Target = NominalSourceCallablePayloadV1;
    fn deref(&self) -> &Self::Target {
        &self.source_signature
    }
}
impl WireEncode for ProtectedCallablePayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.source_signature.encode(encoder)
    }
}
