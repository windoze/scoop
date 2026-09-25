use super::*;
use crate::production::nominal_interfaces::owner_resolution;

impl Projection<'_> {
    pub(super) fn method_owner(
        &mut self,
        method: Method,
        source: SourceNominalId,
    ) -> Result<(), Error> {
        if owner_resolution::from_type(self.export, method.owner) != Some(source) {
            return Err(invalid(
                "nominal callable identity disagrees with its method owner",
            ));
        }
        Ok(())
    }
    pub(super) fn slots(&mut self, method: Method) -> Result<CanonicalProtectedSlotRefsV1, Error> {
        let slot = match method.dispatch {
            MethodDispatch::Direct => None,
            MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => Some(
                self.export
                    .dispatch_slot_identities
                    .get_virtual(family)
                    .ok_or_else(|| {
                        invalid("nominal callable virtual family has no sealed slot identity")
                    })?
                    .id(),
            ),
            MethodDispatch::Interface(member) => Some(
                self.export
                    .dispatch_slot_identities
                    .get_interface(member)
                    .ok_or_else(|| {
                        invalid("nominal callable interface member has no sealed slot identity")
                    })?
                    .id(),
            ),
        };

        CanonicalProtectedSlotRefsV1::try_new(slot.into_iter().collect()).map_err(invalid)
    }
    pub(super) fn modality(
        &self,
        function: FunctionId,
        method: Method,
    ) -> Result<CallableModalityV1, Error> {
        if let MethodDispatch::Interface(member) = method.dispatch {
            let member = &self.export.interface_methods[member];
            if member.function != function {
                return Err(invalid(
                    "nominal callable interface member disagrees with its function",
                ));
            }
            return Ok(match member.implementation {
                InterfaceMemberImplementation::Body => CallableModalityV1::InterfaceDefault,
                InterfaceMemberImplementation::AbstractSlot => CallableModalityV1::Abstract,
            });
        }
        Ok(match method.modifier {
            MethodModifier::Final => CallableModalityV1::Final,
            MethodModifier::Open => CallableModalityV1::Open,
            MethodModifier::Abstract => CallableModalityV1::Abstract,
        })
    }
}
