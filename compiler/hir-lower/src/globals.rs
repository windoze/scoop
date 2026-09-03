//! M12 top-level local/TLS storage and C data imports.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::call_resolution::applicability::NominalApplicabilityInput;
use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};

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
            (hir::Type::Ptr(pointee), ast::Expr::Call(call)) => {
                let structure = self.global_struct_callee(call)?;
                if Some(structure) != self.ffi_ptr {
                    return None;
                }
                let constructor = self.struct_primary_constructor(structure)?;
                let view =
                    self.nominal_constructor_view(NominalConstructorSource::Struct(constructor));
                let (values, _) = self.global_nominal_constant(&view, call, &[pointee])?;
                matches!(values.as_slice(), [hir::ConstantValue::Int(0)])
                    .then_some(hir::ConstantValue::NullPtr)
            }
            (hir::Type::FunPtr(signature), ast::Expr::Call(call)) => {
                let structure = self.global_struct_callee(call)?;
                if Some(structure) != self.ffi_fun_ptr {
                    return None;
                }
                let constructor = self.struct_primary_constructor(structure)?;
                let mut view =
                    self.nominal_constructor_view(NominalConstructorSource::Struct(constructor));
                view.value_parameters.clear();
                let function_ty = self.function_types[signature].canonical_type;
                self.global_nominal_constant(&view, call, &[function_ty])?;
                Some(hir::ConstantValue::NullFunPtr)
            }
            (hir::Type::Struct(application), ast::Expr::Call(call)) => {
                let application_value = self.struct_applications[application].clone();
                let struct_id = application_value.template;
                if self.global_struct_callee(call) != Some(struct_id) {
                    return None;
                }
                let constructor = self.struct_primary_constructor(struct_id)?;
                let view =
                    self.nominal_constructor_view(NominalConstructorSource::Struct(constructor));
                let (values, _) =
                    self.global_nominal_constant(&view, call, &application_value.arguments)?;
                Some(hir::ConstantValue::Struct {
                    application,
                    fields: values,
                })
            }
            _ => None,
        }
    }

    fn global_struct_callee(&self, call: &ast::CallExpr) -> Option<hir::StructId> {
        self.structs_by_name
            .get(&call.callee.text)
            .map(|&(structure, _)| structure)
    }

    fn global_nominal_constant(
        &mut self,
        view: &NominalConstructorView,
        call: &ast::CallExpr,
        expected_arguments: &[hir::TypeId],
    ) -> Option<(Vec<hir::ConstantValue>, Vec<hir::TypeId>)> {
        if expected_arguments.len() != view.owner_parameters.len() {
            return None;
        }
        let argument_map = CandidateArgumentMap::source_nominal(view, &call.args).ok()?;
        let explicit_arguments = self.resolve_call_type_args(&call.type_args)?;
        if !explicit_arguments.is_empty() && explicit_arguments.len() != view.owner_parameters.len()
        {
            return None;
        }
        let seed = if explicit_arguments.is_empty() {
            expected_arguments
        } else {
            &explicit_arguments
        };
        let parameter_types = view
            .value_parameters
            .iter()
            .map(|parameter| self.instantiate_ty(parameter.ty, seed))
            .collect::<Vec<_>>();
        let values = call
            .args
            .iter()
            .zip(&parameter_types)
            .map(|(argument, &parameter)| self.global_constant(&argument.expression, parameter))
            .collect::<Option<Vec<_>>>()?;
        let argument_types = parameter_types
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>();
        let solution = self
            .solve_nominal_applicability(NominalApplicabilityInput {
                view,
                argument_map: &argument_map,
                explicit_arguments: &explicit_arguments,
                expected_arguments: Some(expected_arguments),
                argument_types: &argument_types,
            })
            .ok()?;
        solution
            .iter()
            .zip(expected_arguments)
            .all(|(&actual, &expected)| self.types_equal(actual, expected))
            .then_some((values, solution))
    }
}
