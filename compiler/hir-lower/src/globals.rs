//! M12 top-level local/TLS storage and C data imports.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn resolve_globals(&mut self, pending: &[(&ast::GlobalDecl, usize)]) {
        let mut initializers = Vec::new();
        for &(decl, file_index) in pending {
            self.current_file = file_index;
            if self.globals_by_name.contains_key(&decl.name.text) {
                self.error(
                    decl.name.span,
                    format!("duplicate global `{}`", decl.name.text),
                );
                continue;
            }
            self.type_params_in_scope.clear();
            let ty = self.resolve_type_ref(&decl.ty).unwrap_or(self.int);
            let checked = self.check_global_annotations(decl);
            let storage = if let Some(extern_) = checked.extern_ {
                hir::GlobalStorage::Extern {
                    library: extern_.library,
                    native_symbol: extern_.native_symbol,
                    thread_local: checked.storage.unwrap_or(false),
                }
            } else {
                hir::GlobalStorage::Local {
                    thread_local: checked.storage.unwrap_or(false),
                    initializer: hir::ConstantValue::Int(0),
                }
            };
            let id = self.globals.alloc(hir::Global {
                name: decl.name.text.clone(),
                ty,
                mutable: decl.mutable,
                storage,
                span: decl.span,
            });
            self.globals_by_name.insert(decl.name.text.clone(), id);
            self.global_files.insert(id, file_index);
            initializers.push((id, decl));
        }

        // Every global name and type is available before constants are
        // inspected. References to another global are nevertheless rejected:
        // initialization has no runtime ordering phase in M12.
        for (id, decl) in initializers {
            self.current_file = self.global_files[&id];
            let global = self.globals[id].clone();
            match global.storage {
                hir::GlobalStorage::Extern { .. } => {
                    if let Err(reason) = self.validate_c_global_type(global.ty, &global.name) {
                        self.error(
                            decl.ty.span,
                            format!("extern global type is not C-FFI-safe: {reason}"),
                        );
                    }
                }
                hir::GlobalStorage::Local { thread_local, .. } => {
                    if self.type_contains_param(global.ty) || !self.is_gc_free(global.ty) {
                        self.error(
                            decl.ty.span,
                            format!(
                                "global storage requires a concrete GC-free value type, found {}",
                                self.type_name(global.ty)
                            ),
                        );
                    }
                    if let Some(expr) = &decl.init {
                        if let Some(initializer) = self.global_constant(expr, global.ty) {
                            self.globals[id].storage = hir::GlobalStorage::Local {
                                thread_local,
                                initializer,
                            };
                        } else {
                            self.error(
                                expr.span(),
                                "global initializer must be a GC-free compile-time constant and must not call functions or read another global"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
    }

    fn global_constant(
        &mut self,
        expr: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::ConstantValue> {
        match (self.types[expected].clone(), expr) {
            (hir::Type::Int | hir::Type::UInt, ast::Expr::IntLiteral { value, .. }) => {
                Some(hir::ConstantValue::Int(*value))
            }
            (
                hir::Type::Int,
                ast::Expr::Unary {
                    op: ast::UnOp::Neg,
                    operand,
                    ..
                },
            ) => match &**operand {
                ast::Expr::IntLiteral { value, .. } => {
                    value.checked_neg().map(hir::ConstantValue::Int)
                }
                _ => None,
            },
            (hir::Type::Boolean, ast::Expr::BoolLiteral { value, .. }) => {
                Some(hir::ConstantValue::Bool(*value))
            }
            (hir::Type::Ptr(pointee), ast::Expr::Call(call))
                if call.callee.text == "Ptr"
                    && call.args.len() == 1
                    && matches!(call.args[0], ast::Expr::IntLiteral { value: 0, .. }) =>
            {
                if self.explicit_global_type_arg_matches(&call.type_args, &[pointee]) {
                    Some(hir::ConstantValue::NullPtr)
                } else {
                    None
                }
            }
            (hir::Type::FunPtr(signature), ast::Expr::Call(call))
                if call.callee.text == "FunPtr" && call.args.is_empty() =>
            {
                let function_ty = self.intern_type(hir::Type::Function(signature));
                if self.explicit_global_type_arg_matches(&call.type_args, &[function_ty]) {
                    Some(hir::ConstantValue::NullFunPtr)
                } else {
                    None
                }
            }
            (hir::Type::Struct(application), ast::Expr::Call(call))
                if call.callee.text
                    == self.structs[self.struct_applications[application].template].name =>
            {
                let application_value = self.struct_applications[application].clone();
                let struct_id = application_value.template;
                let type_args = application_value.arguments;
                if !self.explicit_global_type_arg_matches(&call.type_args, &type_args) {
                    return None;
                }
                let fields = self.structs[struct_id].semantic_fields().to_vec();
                if call.args.len() != fields.len() {
                    return None;
                }
                let mut values = Vec::with_capacity(fields.len());
                for (argument, field) in call.args.iter().zip(fields) {
                    let field_ty = self.instantiate_ty(field.ty, &type_args);
                    values.push(self.global_constant(argument, field_ty)?);
                }
                Some(hir::ConstantValue::Struct {
                    application,
                    fields: values,
                })
            }
            _ => None,
        }
    }

    fn explicit_global_type_arg_matches(
        &mut self,
        refs: &[ast::TypeRef],
        expected: &[hir::TypeId],
    ) -> bool {
        if refs.is_empty() {
            return true;
        }
        if refs.len() != expected.len() {
            return false;
        }
        refs.iter().zip(expected).all(|(reference, &expected)| {
            self.resolve_type_ref(reference)
                .is_some_and(|actual| self.types_equal(actual, expected))
        })
    }
}
