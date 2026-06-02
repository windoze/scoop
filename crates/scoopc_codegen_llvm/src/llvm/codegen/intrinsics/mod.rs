//! Builtin and sysroot intrinsic lowering split out of `codegen/mod.rs`.

use super::*;

mod atomic;
mod builtin;
mod named;
mod sysroot;

pub(in crate::llvm::codegen) use named::scalar_bodyless_intrinsic_entry_name;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn panic_verified_intrinsic_contract(
        &self,
        context: &'static str,
        detail: &'static str,
    ) -> ! {
        panic!("{context}: verified intrinsic contract was violated: {detail}")
    }

    pub(in crate::llvm::codegen) fn expect_hir_intrinsic_arity(
        &self,
        args: &[hir::CallArg],
        expected: usize,
        context: &'static str,
    ) {
        if args.len() != expected {
            self.panic_verified_intrinsic_contract(context, "argument count drift");
        }
    }

    pub(in crate::llvm::codegen) fn expect_hir_positional_intrinsic_arg<'b>(
        &self,
        args: &'b [hir::CallArg],
        expected: usize,
        index: usize,
        context: &'static str,
    ) -> &'b hir::Expr {
        self.expect_hir_intrinsic_arity(args, expected, context);
        let Some(arg) = args.get(index) else {
            self.panic_verified_intrinsic_contract(context, "argument index drift");
        };
        let hir::CallArg::Positional(expr) = arg else {
            self.panic_verified_intrinsic_contract(context, "named argument drift");
        };
        expr
    }

    pub(in crate::llvm::codegen) fn expect_mir_intrinsic_arity(
        &self,
        args: &[crate::effect_lowered::mir_source::CallArg],
        expected: usize,
        context: &'static str,
    ) {
        if args.len() != expected {
            self.panic_verified_intrinsic_contract(context, "argument count drift");
        }
    }

    pub(in crate::llvm::codegen) fn expect_mir_positional_intrinsic_arg<'b>(
        &self,
        args: &'b [crate::effect_lowered::mir_source::CallArg],
        expected: usize,
        index: usize,
        context: &'static str,
    ) -> &'b crate::effect_lowered::mir_source::CallArg {
        self.expect_mir_intrinsic_arity(args, expected, context);
        let Some(arg) = args.get(index) else {
            self.panic_verified_intrinsic_contract(context, "argument index drift");
        };
        if arg.name.is_some() {
            self.panic_verified_intrinsic_contract(context, "named argument drift");
        }
        arg
    }
}
