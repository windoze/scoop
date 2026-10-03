use super::*;

impl DecodedExactDispatchImplementationV1 {
    pub(super) fn read_link(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExactDispatchImplementationV1, LinkDataError> {
        Ok(match *self {
            Self::AbstractObligation {
                declaration,
                trap_target,
                ref receiver,
            } => ExactDispatchImplementationV1::AbstractObligation {
                declaration: declaration.resolve(identities).map_err(link_error)?,
                trap_target: trap_target.resolve(identities).map_err(link_error)?,
                receiver: receiver.read_link(),
            },
            Self::DirectStrongTarget {
                target,
                ref receiver,
            } => ExactDispatchImplementationV1::DirectStrongTarget {
                target: target.resolve(identities).map_err(link_error)?,
                receiver: receiver.read_link(),
            },
            Self::InterfaceDefaultTarget {
                target,
                ref receiver,
            } => ExactDispatchImplementationV1::InterfaceDefaultTarget {
                target: target.resolve(identities).map_err(link_error)?,
                receiver: receiver.read_link(),
            },
            Self::AdjustThunkTarget(target) => ExactDispatchImplementationV1::AdjustThunkTarget(
                target.resolve(identities).map_err(link_error)?,
            ),
        })
    }
}

impl DecodedExactDispatchReceiverAdaptationV1 {
    fn read_link(&self) -> ExactDispatchReceiverAdaptationV1 {
        match self {
            Self::Identity => ExactDispatchReceiverAdaptationV1::Identity,
            Self::ReferenceDispatch => ExactDispatchReceiverAdaptationV1::ReferenceDispatch,
        }
    }
}
