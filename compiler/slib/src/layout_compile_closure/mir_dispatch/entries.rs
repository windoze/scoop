use super::*;
use mir::{
    MirDispatchImplementationV1 as Implementation, MirDispatchReceiverAdaptationV1 as Receiver,
};

impl Replay<'_, '_> {
    pub(super) fn entry(
        &mut self,
        (owner, value): (PersistentExactTypeId, bool),
        role: hir::InheritanceSlotSchemaRoleV1,
        contract: &hir::InheritanceSlotContractV1,
        candidate: &mir::MirDispatchEntryV1,
    ) -> Result<(), Error> {
        let source = contract.signature();
        let receiver = match role {
            hir::InheritanceSlotSchemaRoleV1::ClassVtable => {
                source.exact_signature().receiver().into_option()
            }
            hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
                Some(interface_exact)
            }
        };
        let signature = bindings::signature(source, receiver)?;

        Error::entry(
            owner,
            contract.slot(),
            Component::Signature,
            candidate.signature() == &signature,
        )?;
        let expected = match contract.implementation() {
            hir::InheritanceSlotImplementationV1::Abstract => {
                let (target, receiver) = self.abstract_target(owner, contract)?;
                let expected = bindings::signature(source, Some(receiver))?;
                self.source_binding(owner, contract.slot(), target, &expected)?;
                let binding = self.binding(target)?;
                Error::entry(
                    owner,
                    contract.slot(),
                    Component::CallableRole,
                    binding.lowering_role()
                        == &mir::MirCallableLoweringRoleV1::PureVirtualTrap {
                            slot: contract.slot(),
                        },
                )?;
                Implementation::AbstractObligation {
                    declaration: declaration(contract.declaration()),
                    trap_target: target,
                    receiver: adaptation(&signature, &expected),
                }
            }
            hir::InheritanceSlotImplementationV1::Concrete(source)
            | hir::InheritanceSlotImplementationV1::InterfaceDefault(source) => {
                if value {
                    let hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } = role
                    else {
                        return Err(Error::SourceSlot {
                            owner,
                            slot: contract.slot(),
                        });
                    };
                    Implementation::AdjustThunkTarget(self.adjustment(
                        owner,
                        contract.slot(),
                        interface_exact,
                        source,
                    )?)
                } else {
                    let expected = bindings::signature(
                        source.signature(),
                        source
                            .signature()
                            .exact_signature()
                            .receiver()
                            .into_option(),
                    )?;
                    let target = target(source.declaration());
                    self.source_binding(owner, contract.slot(), target, &expected)?;
                    let receiver = adaptation(&signature, &expected);
                    if matches!(
                        contract.implementation(),
                        hir::InheritanceSlotImplementationV1::InterfaceDefault(_)
                    ) {
                        Implementation::InterfaceDefaultTarget { target, receiver }
                    } else {
                        Implementation::DirectStrongTarget { target, receiver }
                    }
                }
            }
        };
        Error::entry(
            owner,
            contract.slot(),
            Component::Implementation,
            candidate.implementation() == expected,
        )
    }
}

fn adaptation(
    slot: &mir::MirBridgeCallableSignatureV1,
    target: &mir::MirBridgeCallableSignatureV1,
) -> Receiver {
    if slot == target {
        Receiver::Identity
    } else {
        Receiver::ReferenceDispatch
    }
}
