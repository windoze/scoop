use super::*;

impl DecodedExactDispatchEntryV1 {
    pub(super) fn read(
        self,
        layouts: &[&CanonicalExactLayoutExportsV1],
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExactDispatchEntryV1, LinkDataError> {
        let slot = identities.resolve(self.slot).map_err(link_error)?;
        let slot_signature = ExactDispatchSlotSignatureV1::new(
            self.slot_signature
                .exact
                .resolve(identities)
                .map_err(link_error)?,
            self.slot_signature.gc_effect,
        );
        let implementation = self.implementation.read(identities)?;
        let slot_receiver_layout = match implementation.receiver_adaptation() {
            ExactDispatchReceiverAdaptationV1::Identity => None,
            ExactDispatchReceiverAdaptationV1::ReferenceDispatch => {
                let exact = slot_signature
                    .exact()
                    .receiver()
                    .into_option()
                    .ok_or_else(|| link_error("reference dispatch has no receiver"))?;
                Some(
                    layouts
                        .iter()
                        .flat_map(|table| table.records())
                        .find_map(|layout| {
                            (layout.identity().exact() == exact
                                && layout.identity().layout_key().representation()
                                    == RepresentationRole::ManagedValue)
                                .then(|| layout.value_handle())
                                .flatten()
                        })
                        .ok_or_else(|| {
                            link_error(format!("missing dispatch receiver layout {exact}"))
                        })?,
                )
            }
        };
        Ok(ExactDispatchEntryV1::from_parts(
            ExactDispatchEntryPartsV1 {
                position: ExactDispatchPositionV1::from_u32(self.position),
                slot,
                slot_signature,
                implementation,
                abi: self.abi.resolve(identities).map_err(link_error)?,
                slot_receiver_layout,
            },
        ))
    }
}

impl DecodedExactDispatchReceiverAdaptationV1 {
    fn read(self) -> ExactDispatchReceiverAdaptationV1 {
        match self {
            Self::Identity => ExactDispatchReceiverAdaptationV1::Identity,
            Self::ReferenceDispatch => ExactDispatchReceiverAdaptationV1::ReferenceDispatch,
        }
    }
}

impl DecodedExactDispatchImplementationV1 {
    fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExactDispatchImplementationV1, LinkDataError> {
        Ok(match self {
            Self::AbstractObligation {
                declaration,
                trap_target,
                receiver,
            } => ExactDispatchImplementationV1::AbstractObligation {
                declaration: declaration.resolve(identities).map_err(link_error)?,
                trap_target: trap_target.resolve(identities).map_err(link_error)?,
                receiver: receiver.read(),
            },
            Self::DirectStrongTarget { target, receiver } => {
                ExactDispatchImplementationV1::DirectStrongTarget {
                    target: target.resolve(identities).map_err(link_error)?,
                    receiver: receiver.read(),
                }
            }
            Self::InterfaceDefaultTarget { target, receiver } => {
                ExactDispatchImplementationV1::InterfaceDefaultTarget {
                    target: target.resolve(identities).map_err(link_error)?,
                    receiver: receiver.read(),
                }
            }
            Self::AdjustThunkTarget(target) => ExactDispatchImplementationV1::AdjustThunkTarget(
                target.resolve(identities).map_err(link_error)?,
            ),
        })
    }
}
