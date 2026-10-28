use super::*;
use inkwell::values::GlobalValue;

impl<'ctx> DebugContext<'_, 'ctx> {
    pub(crate) fn global(&self, binding: &scoop_lir::DebugGlobal, global: GlobalValue<'ctx>) {
        if !self.full {
            return;
        }
        let source = &binding.location;
        let variable = self.builder.create_global_variable_expression(
            self.unit.as_debug_info_scope(),
            &binding.name,
            global
                .get_name()
                .to_str()
                .expect("persistent storage symbols are UTF-8"),
            self.file(&source.file),
            source.line,
            self.source_type(binding.ty),
            false,
            None,
            None,
            global.get_alignment() * 8,
        );
        global.set_metadata(
            variable.as_metadata_value(self.context),
            self.context.get_kind_id("dbg"),
        );
    }
}
