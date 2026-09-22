use super::*;

impl Context<'_> {
    pub(super) fn entries(
        &self,
        owner: PersistentExactTypeId,
        schema: &hir::InheritanceSlotSchemaV1,
        targets: &[StrongCallableDefinitionOwner],
        meter: &mut BudgetMeter,
    ) -> Result<Vec<mir::MirDispatchEntryV1>, Error> {
        let mut entries = reserve(schema.slots().len(), meter)?;
        for (position, (slot, target)) in schema.slots().iter().zip(targets).enumerate() {
            work(
                search(self.source.slot_selections().records().len()) + 1,
                meter,
            )?;
            let selection = self
                .source
                .slot_selections()
                .get(owner, *slot)
                .ok_or(Error::MissingSelection { owner, slot: *slot })?;
            let key = self
                .authority
                .identities
                .canonical_key::<_, DispatchSlotKey>(*slot)?;
            let root = self.callable(declaration_target(key.owner()), meter)?;
            let exact = root.lowered_signature().exact();
            let receiver = match schema.role() {
                hir::InheritanceSlotSchemaRoleV1::ClassVtable => exact.receiver().into_option(),
                hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
                    Some(interface_exact)
                }
            };
            let mut parameters = reserve(exact.parameters().len(), meter)?;
            parameters.extend_from_slice(exact.parameters());
            work(parameters.len() as u64 + 1, meter)?;
            let signature = mir::MirBridgeCallableSignatureV1::new(
                ExactCallableSignature::new(exact.effect(), receiver, parameters, exact.result()),
                root.lowered_signature().gc_effect(),
            );
            let binding = self.callable(*target, meter)?;
            let mismatch = || Error::TargetMismatch { owner, slot: *slot };
            let receiver = if signature == *binding.lowered_signature() {
                mir::MirDispatchReceiverAdaptationV1::Identity
            } else {
                mir::MirDispatchReceiverAdaptationV1::ReferenceDispatch
            };
            let implementation = match selection {
                hir::InheritanceSourceSlotSelectionV1::Abstract => {
                    if *binding.lowering_role()
                        != (mir::MirCallableLoweringRoleV1::PureVirtualTrap { slot: *slot })
                    {
                        return Err(mismatch());
                    }
                    mir::MirDispatchImplementationV1::AbstractObligation {
                        declaration: key.owner(),
                        trap_target: *target,
                        receiver,
                    }
                }
                hir::InheritanceSourceSlotSelectionV1::Concrete(expected)
                | hir::InheritanceSourceSlotSelectionV1::InterfaceDefault(expected) => {
                    let expected = source_target(expected);
                    if let mir::MirCallableLoweringRoleV1::BoxingAdjust { target: actual }
                    | mir::MirCallableLoweringRoleV1::DispatchAdjust { target: actual } =
                        *binding.lowering_role()
                    {
                        if actual != expected {
                            return Err(mismatch());
                        }
                        mir::MirDispatchImplementationV1::AdjustThunkTarget(*target)
                    } else {
                        if *target != expected {
                            return Err(mismatch());
                        }
                        if matches!(
                            selection,
                            hir::InheritanceSourceSlotSelectionV1::InterfaceDefault(_)
                        ) {
                            mir::MirDispatchImplementationV1::InterfaceDefaultTarget {
                                target: *target,
                                receiver,
                            }
                        } else {
                            mir::MirDispatchImplementationV1::DirectStrongTarget {
                                target: *target,
                                receiver,
                            }
                        }
                    }
                }
            };
            entries.push(mir::MirDispatchEntryV1::new(
                *slot,
                mir::MirDispatchPositionV1::new(u32::try_from(position).map_err(|_| mismatch())?),
                signature,
                implementation,
            ));
        }
        Ok(entries)
    }
}
