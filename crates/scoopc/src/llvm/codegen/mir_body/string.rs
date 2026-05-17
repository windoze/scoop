//! MIR string-related value helpers.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_unresolved_name_with_source_ty(
        &mut self,
        span: crate::span::Span,
        name: &str,
        source_types: &TypeStore,
        source_ty: TypeId,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_cg = self.cg_ty_of_mir_type(source_types, source_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "MIR unresolved name source type",
                at: span.into(),
            },
        )?;
        let value = self.codegen_unresolved_ident(span, name, Some(source_cg))?;
        self.coerce_value(span, value, target_cg)
    }
}
