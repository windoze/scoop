use super::*;

mod access;
mod allocation;
mod assembly;
mod clone;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_array_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        match instruction {
            Instruction::ArrayAlloc { .. } => self.emit_array_allocation(instruction),
            Instruction::ArrayAssembly { .. } => self.emit_array_assembly(instruction),
            Instruction::ArrayLen { .. }
            | Instruction::ArrayGet { .. }
            | Instruction::ArraySet { .. } => self.emit_array_access(instruction),
            Instruction::ArrayClone { .. } => self.emit_array_clone(instruction),
            _ => unreachable!("instruction dispatcher routes only array instructions"),
        }
    }
}
