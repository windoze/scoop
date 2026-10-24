//! Omit dead zero stores before definite, unobservable field initialization.

use super::*;

impl FnEmitter<'_, '_> {
    pub(super) fn initialization_clear_size(
        &mut self,
        out: TempId,
        size: u64,
    ) -> Result<u64, CodegenError> {
        if size == 16 || size > self.allocation_initialization_limit {
            return Ok(size);
        }
        if self.allocation_addressed_locals.is_none() {
            self.allocation_addressed_locals = Some(
                self.function
                    .blocks
                    .iter()
                    .flat_map(|(_, block)| &block.instructions)
                    .filter_map(|instruction| match instruction {
                        Instruction::LocalAddress { local, .. } => Some(*local),
                        _ => None,
                    })
                    .collect(),
            );
        }
        let addressed = self
            .allocation_addressed_locals
            .as_ref()
            .expect("addressed locals were collected for this function");
        let mut aliases = HashSet::from([Value::Temp(out)]);
        let mut covered = vec![false; size as usize];
        let mut block = self.current_block;
        let mut start = self.current_instruction + 1;
        let mut visited = HashSet::from([block]);
        'path: loop {
            for instruction in &self.function.blocks[block].instructions[start..] {
                match instruction {
                    Instruction::Store { local, value } if !addressed.contains(local) => {
                        if aliases.contains(value) {
                            aliases.insert(Value::Local(*local));
                        } else {
                            aliases.remove(&Value::Local(*local));
                        }
                    }
                    Instruction::HeapStore {
                        object,
                        offset,
                        value,
                    } if aliases.contains(object) => {
                        let ty = self.function.value_ty(self.globals_arena, *value);
                        let Some(bytes) = self.dense_store_size(&ty)? else {
                            // The store is still into this unpublished object.
                            // Its reference slots and padding remain zeroed.
                            continue;
                        };
                        let Some(end) = offset.checked_add(bytes).filter(|end| *end <= size) else {
                            break 'path;
                        };
                        if *offset < 16 {
                            break 'path;
                        }
                        covered[*offset as usize..end as usize].fill(true);
                    }
                    Instruction::MakeZstValue { .. } => {}
                    Instruction::MakeAggregate { elements, .. }
                        if elements.iter().all(|value| !aliases.contains(value)) => {}
                    Instruction::ExtractValue { aggregate, .. } if !aliases.contains(aggregate) => {
                    }
                    _ => break 'path,
                }
            }
            let Terminator::Br(next) = self.function.blocks[block].terminator else {
                break;
            };
            if !visited.insert(next) {
                break;
            }
            block = next;
            start = 0;
        }

        // Keep one memset, including the header, every reference slot and all
        // padding. Only a completely overwritten suffix can shorten it.
        let mut clear_size = size;
        while clear_size > 16 && covered[clear_size as usize - 1] {
            clear_size -= 1;
        }
        Ok(clear_size)
    }

    /// An LLVM aggregate store need not write its padding. Only count types
    /// whose scalar leaves densely cover their complete, GC-free storage.
    fn dense_store_size(&self, ty: &LirType) -> Result<Option<u64>, CodegenError> {
        Ok(match ty {
            LirType::I1 | LirType::I8 => Some(1),
            LirType::I16 => Some(2),
            LirType::I32 | LirType::F32 => Some(4),
            LirType::I64
            | LirType::F64
            | LirType::Ptr(PointerKind::Raw | PointerKind::Code | PointerKind::Metadata) => Some(8),
            LirType::Struct(id) => {
                let definition = &self.structs[*id];
                let StructRepresentation::Scoop { fields } = &definition.representation else {
                    return Ok(None);
                };
                let mut end = 0;
                for field in fields {
                    let Some(bytes) = self.dense_store_size(&field.ty)? else {
                        return Ok(None);
                    };
                    if field.layout.offset != end {
                        return Ok(None);
                    }
                    end += bytes;
                }
                (end == definition.size).then_some(end)
            }
            LirType::Aggregate(fields) => {
                let layout = basic_ty(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    ty,
                )?
                .into_struct_type();
                let mut end = 0;
                for (index, field) in fields.iter().enumerate() {
                    let Some(bytes) = self.dense_store_size(field)? else {
                        return Ok(None);
                    };
                    if self.target_data.offset_of_element(&layout, index as u32) != Some(end) {
                        return Ok(None);
                    }
                    end += bytes;
                }
                (end == self.target_data.get_abi_size(&layout)).then_some(end)
            }
            LirType::Void
            | LirType::MachineScalar(_)
            | LirType::Ptr(PointerKind::Managed)
            | LirType::Interface
            | LirType::ExceptionRecord
            | LirType::Enum(_) => None,
        })
    }
}
