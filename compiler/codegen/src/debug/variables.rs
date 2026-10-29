use super::*;
use inkwell::basic_block::BasicBlock;
use inkwell::debug_info::{DIExpression, DILocalVariable};
use inkwell::values::{AsValueRef, BasicValueEnum, PointerValue};
use scoop_lir::{DebugBinding, DebugStorage};

pub(super) struct Variable<'ctx> {
    variable: DILocalVariable<'ctx>,
    location: DILocation<'ctx>,
    managed: bool,
}

impl<'ctx> FunctionDebug<'_, 'ctx> {
    pub(crate) fn is_full(&self) -> bool {
        self.context.full
    }

    pub(crate) fn observes(&self, storage: DebugStorage) -> bool {
        self.variables
            .get(&storage)
            .is_some_and(|variables| variables.iter().any(|variable| variable.managed))
    }

    pub(crate) fn register(&mut self, binding: &DebugBinding, managed: bool) {
        let location = self.location(&DebugOrigin::Source(binding.location.clone()));
        let scope = if binding.parameter.is_some() {
            match &binding.location.inlined_at {
                Some(site) => self.inline_functions[site],
                None => self.scope,
            }
        } else {
            location.get_scope()
        };
        let file = self.context.file(&binding.location.file);
        let ty = self.context.source_type(binding.ty);
        let key = (
            binding.declaration.clone(),
            scope.as_mut_ptr() as usize,
            binding.ty,
        );
        let variable =
            *self
                .variable_definitions
                .entry(key)
                .or_insert_with(|| match binding.parameter {
                    Some(parameter) => self.context.builder.create_parameter_variable(
                        scope,
                        &binding.declaration.name,
                        parameter.get(),
                        file,
                        binding.location.line,
                        ty,
                        true,
                        DIFlags::PUBLIC,
                    ),
                    None => self.context.builder.create_auto_variable(
                        scope,
                        &binding.declaration.name,
                        file,
                        binding.location.line,
                        ty,
                        true,
                        DIFlags::PUBLIC,
                        ty.get_align_in_bits(),
                    ),
                });
        self.variables
            .entry(binding.storage)
            .or_default()
            .push(Variable {
                variable,
                location,
                managed,
            });
    }

    pub(crate) fn declare(
        &self,
        storage: DebugStorage,
        pointer: PointerValue<'ctx>,
        block: BasicBlock<'ctx>,
    ) {
        if let Some(variables) = self.variables.get(&storage) {
            for variable in variables {
                let expression = self.context.builder.create_expression(Vec::new());
                // LLVM 22 returns a DbgRecord, not an InstructionValue. Keep
                // the native record API rather than casting its return value.
                unsafe {
                    inkwell::llvm_sys::debuginfo::LLVMDIBuilderInsertDeclareRecordAtEnd(
                        self.context.builder.as_mut_ptr(),
                        pointer.as_value_ref(),
                        variable.variable.as_mut_ptr(),
                        expression.as_mut_ptr(),
                        variable.location.as_mut_ptr(),
                        block.as_mut_ptr(),
                    );
                }
            }
        }
    }

    pub(crate) fn value(
        &self,
        storage: DebugStorage,
        value: BasicValueEnum<'ctx>,
        fragment: Option<(u64, u64)>,
        block: BasicBlock<'ctx>,
    ) {
        if let Some(variables) = self.variables.get(&storage) {
            let expression = self.context.builder.create_expression(match fragment {
                // DW_OP_LLVM_fragment uses bit offset and bit size.
                Some((offset, size)) => vec![0x1000, (offset * 8) as i64, (size * 8) as i64],
                None => Vec::new(),
            });
            for variable in variables {
                self.value_record(variable, value, expression, block);
            }
        }
    }

    pub(crate) fn invalidate_managed(&self, block: BasicBlock<'ctx>) {
        let unavailable = self.context.context.i8_type().get_undef().into();
        let expression = self.context.builder.create_expression(Vec::new());
        for variable in self
            .variables
            .values()
            .flatten()
            .filter(|variable| variable.managed)
        {
            self.value_record(variable, unavailable, expression, block);
        }
    }

    pub(crate) fn address(
        &self,
        storage: DebugStorage,
        pointer: PointerValue<'ctx>,
        block: BasicBlock<'ctx>,
    ) {
        if let Some(variables) = self.variables.get(&storage) {
            // A value observation through existing storage, not a lifetime-wide
            // declare: a later safepoint can end this location explicitly.
            let expression = self.context.builder.create_expression(vec![0x06]); // DW_OP_deref
            for variable in variables {
                self.value_record(variable, pointer.into(), expression, block);
            }
        }
    }

    fn value_record(
        &self,
        variable: &Variable<'ctx>,
        value: BasicValueEnum<'ctx>,
        expression: DIExpression<'ctx>,
        block: BasicBlock<'ctx>,
    ) {
        unsafe {
            inkwell::llvm_sys::debuginfo::LLVMDIBuilderInsertDbgValueRecordAtEnd(
                self.context.builder.as_mut_ptr(),
                value.as_value_ref(),
                variable.variable.as_mut_ptr(),
                expression.as_mut_ptr(),
                variable.location.as_mut_ptr(),
                block.as_mut_ptr(),
            );
        }
    }
}
