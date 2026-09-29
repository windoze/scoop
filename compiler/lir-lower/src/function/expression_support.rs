use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn trap_message(&mut self, message: &str) -> lir::GlobalId {
        if let Some(&global) = self.trap_messages.get(message) {
            return global;
        }
        let path = scoop_identity::StructuralDefinitionPath::from_first(
            scoop_identity::StructuralPathSegment::new(
                scoop_identity::StructuralDefinitionSiteRole::StringConstant,
                self.cstr_count,
            ),
            [],
        );
        self.cstr_count = self
            .cstr_count
            .checked_add(1)
            .expect("one callable cannot contain more than u32::MAX trap strings");
        let identity = lir::CallableCStringIdentity::new(self.producer, self.callable_body, path)
            .expect("a callable trap string has a canonical associated-atom identity");
        let global = self.globals.alloc(lir::Global {
            address_kind: lir::PointerKind::Raw,
            scan: lir::RefScan::None,
            init: lir::GlobalInit::CString {
                identity,
                value: message.to_string(),
            },
        });
        self.trap_messages.insert(message.to_string(), global);
        global
    }

    /// The shared trap block for `message` in this function (one per
    /// message, created on first use): calls the runtime trap —
    /// `void scoop_rt_trap(ptr)`, noreturn — with the message global
    /// and ends `unreachable`.
    pub(super) fn trap_block(&mut self, message: &str) -> StorageResult<lir::BlockId> {
        if let Some(&block) = self.trap_blocks.get(message) {
            return Ok(block);
        }
        let global = self.trap_message(message);
        let block = self.new_block("unwrap.trap");
        // Fill the trap block out of line; the caller seals the
        // suspended current block with the branch.
        let saved = self.current;
        let saved_sealed = self.current_sealed;
        self.enter(block);
        let (call, result) = self.typed_call(
            vec![lir::RAW_PTR],
            lir::LirType::Void,
            vec![lir::Value::Global(global)],
        )?;
        assert!(result.is_none(), "trap has no value result");
        let site = self.call_site(
            LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Trap),
            call,
        );
        self.push(lir::Instruction::Call { site });
        self.seal(lir::Terminator::Unreachable);
        self.trap_blocks.insert(message.to_string(), block);
        self.current = saved;
        self.current_sealed = saved_sealed;
        Ok(block)
    }

    pub(super) fn td_ref(&self, ty: &mir::Type) -> lir::Value {
        lir::Value::TypeDescriptor(self.type_descriptors.for_type(ty))
    }

    /// Struct / tuple construction: an aggregate of the mapped field
    /// values in declaration order.
    pub(super) fn make_aggregate(
        &mut self,
        ty: &mir::Type,
        elements: Vec<lir::Value>,
    ) -> lir::Value {
        let ty = self.value_type(ty);
        let out = self.new_temp(ty);
        self.push(lir::Instruction::MakeAggregate { out, elements });
        lir::Value::Temp(out)
    }

    /// The Unit value: an empty aggregate (void calls and Unit
    /// literals produce it; void functions never return it).
    pub(super) fn unit_value(&mut self) -> lir::Value {
        let out = self.new_temp(lir::LirType::Aggregate(Vec::new()));
        self.push(lir::Instruction::MakeAggregate {
            out,
            elements: Vec::new(),
        });
        lir::Value::Temp(out)
    }
}
