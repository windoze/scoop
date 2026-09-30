//! M12 C-FFI-safe classification and `@CLayout` validation.

use std::collections::HashSet;

use scoop_hir as hir;

use crate::Lowerer;

mod classification;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Classification {
    Safe,
    /// A generic `@CLayout` definition may mention one of its own type
    /// parameters. Its concrete applications are checked separately.
    Deferred,
}

#[derive(Debug)]
struct CAbiError {
    path: Vec<String>,
    reason: String,
}

impl CAbiError {
    fn render(&self) -> String {
        if self.path.is_empty() {
            self.reason.clone()
        } else {
            format!("{}: {}", self.path.join("."), self.reason)
        }
    }
}

impl Lowerer {
    pub(crate) fn validate_foreign_callback_signature(
        &mut self,
        signature: hir::FunctionTypeId,
        span: scoop_ast::Span,
    ) -> bool {
        let function = self.function_types[signature].clone();
        if function.is_suspend || self.function_type_contains_param(signature) {
            self.error(
                span,
                "foreign callback signature must be ordinary and fully concrete".to_string(),
            );
            return false;
        }
        let mut visiting = HashSet::new();
        for (index, parameter) in function.parameter_types.iter().copied().enumerate() {
            if let Err(error) = self.classify_c_ffi_type(
                parameter,
                &[],
                false,
                vec![format!("parameter{}", index + 1)],
                &mut visiting,
            ) {
                self.error(
                    span,
                    format!(
                        "foreign callback signature is not C-FFI-safe: {}",
                        error.render()
                    ),
                );
                return false;
            }
        }
        if let Err(error) = self.classify_c_ffi_type(
            function.return_type,
            &[],
            true,
            vec!["return".to_string()],
            &mut visiting,
        ) {
            self.error(
                span,
                format!(
                    "foreign callback signature is not C-FFI-safe: {}",
                    error.render()
                ),
            );
            return false;
        }
        true
    }

    pub(crate) fn validate_extern_global_symbols(&mut self) {
        let extern_globals: Vec<_> = self
            .globals
            .iter()
            .filter_map(|(id, global)| match &global.storage {
                hir::GlobalStorage::Extern {
                    library,
                    native_symbol,
                    thread_local,
                } => Some((id, library.clone(), native_symbol.clone(), *thread_local)),
                hir::GlobalStorage::Managed { .. } | hir::GlobalStorage::Local { .. } => None,
            })
            .collect();
        for (index, (id, library, symbol, thread_local)) in extern_globals.iter().enumerate() {
            for (previous, previous_library, previous_symbol, previous_tls) in
                extern_globals.iter().take(index)
            {
                if previous_symbol != symbol {
                    continue;
                }
                let compatible = previous_library == library
                    && previous_tls == thread_local
                    && self.properties[self.globals[*previous].property]
                        .capability
                        .setter()
                        .is_some()
                        == self.properties[self.globals[*id].property]
                            .capability
                            .setter()
                            .is_some()
                    && self.types_equal(self.globals[*previous].ty, self.globals[*id].ty);
                if !compatible {
                    self.current_file = self.property_files[&self.globals[*id].property];
                    self.error(
                        self.globals[*id].span,
                        format!(
                            "extern data symbol `{symbol}` conflicts with global `{}`",
                            self.properties[self.globals[*previous].property].name
                        ),
                    );
                }
                break;
            }
            let conflicting_function = self.functions.iter().find_map(|(_, function)| {
                matches!(
                    function.kind,
                    hir::FunctionKind::Extern(extern_id)
                        if self.extern_functions[extern_id].native_symbol == *symbol
                )
                .then(|| function.name.clone())
            });
            if let Some(function_name) = conflicting_function {
                self.current_file = self.property_files[&self.globals[*id].property];
                self.error(
                    self.globals[*id].span,
                    format!(
                        "extern data symbol `{symbol}` conflicts with function `{}`",
                        function_name
                    ),
                );
            }
        }
    }

    pub(crate) fn validate_c_global_type(
        &mut self,
        ty: hir::TypeId,
        name: &str,
    ) -> Result<(), String> {
        let mut visiting = HashSet::new();
        self.classify_c_ffi_type(ty, &[], false, vec![name.to_string()], &mut visiting)
            .map(|_| ())
            .map_err(|error| error.render())
    }

    /// Check complete extern signatures and native-symbol consistency after
    /// every declaration signature has been resolved.
    pub(crate) fn validate_extern_functions(&mut self) {
        let externs: Vec<_> = self
            .functions
            .iter()
            .filter_map(|(function, declaration)| match declaration.kind {
                hir::FunctionKind::Extern(id) => Some((function, id)),
                _ => None,
            })
            .collect();
        for (function_id, extern_id) in &externs {
            self.current_file = self.function_files[function_id];
            let function = self.functions[*function_id].clone();
            let extern_ = self.extern_functions[*extern_id].clone();
            if extern_.abi == hir::ExternAbi::C
                && self.signatures[function_id].params.iter().any(|parameter| {
                    matches!(parameter.calling, crate::FnParamCalling::Vararg { .. })
                })
            {
                self.error(
                    function.span,
                    format!(
                        "C ABI extern function `{}` cannot declare a language `vararg` parameter",
                        function.name
                    ),
                );
            }
            for (index, parameter) in extern_.params.iter().copied().enumerate() {
                let path = vec![function.name.clone(), format!("parameter{}", index + 1)];
                let result = match extern_.abi {
                    hir::ExternAbi::C => {
                        let mut visiting = HashSet::new();
                        self.classify_c_ffi_type(parameter, &[], false, path, &mut visiting)
                            .map(|_| ())
                    }
                    hir::ExternAbi::Scoop => self.classify_scoop_abi_type(parameter, false, path),
                };
                if let Err(error) = result {
                    self.error(
                        function.span,
                        format!("extern parameter is not ABI-safe: {}", error.render()),
                    );
                }
            }
            let path = vec![function.name.clone(), "return".to_string()];
            let result = match extern_.abi {
                hir::ExternAbi::C => {
                    let mut visiting = HashSet::new();
                    self.classify_c_ffi_type(extern_.return_type, &[], true, path, &mut visiting)
                        .map(|_| ())
                }
                hir::ExternAbi::Scoop => {
                    self.classify_scoop_abi_type(extern_.return_type, true, path)
                }
            };
            if let Err(error) = result {
                self.error(
                    function.span,
                    format!("extern return type is not ABI-safe: {}", error.render()),
                );
            }
        }

        for (index, (function_id, extern_id)) in externs.iter().enumerate() {
            let current = &self.extern_functions[*extern_id];
            for (other_function, other_id) in externs.iter().take(index) {
                let previous = &self.extern_functions[*other_id];
                if previous.native_symbol != current.native_symbol {
                    continue;
                }
                let same_signature = previous.params.len() == current.params.len()
                    && previous
                        .params
                        .iter()
                        .zip(&current.params)
                        .all(|(&left, &right)| self.types_equal(left, right))
                    && self.types_equal(previous.return_type, current.return_type);
                if previous.library != current.library
                    || previous.abi != current.abi
                    || previous.calling_convention != current.calling_convention
                    || previous.gc_effect != current.gc_effect
                    || previous.safety != current.safety
                    || !same_signature
                {
                    self.current_file = self.function_files[function_id];
                    self.error(
                        self.functions[*function_id].span,
                        format!(
                            "extern symbol `{}` conflicts with declaration `{}`",
                            current.native_symbol, self.functions[*other_function].name
                        ),
                    );
                }
                break;
            }
        }
    }

    /// Validate every `@CLayout` definition, every concrete generic
    /// `@CLayout` application materialized while lowering, and every source
    /// `FunPtr` use. This runs after bodies so inferred generic constructor
    /// applications are present in the canonical type arena as well.
    pub(crate) fn validate_c_ffi_types(&mut self) {
        let layouts: Vec<_> = self
            .structs
            .iter()
            .filter_map(|(id, declaration)| declaration.attributes.c_layout.map(|_| id))
            .collect();
        for id in layouts {
            self.current_file = self.struct_files[&id];
            let declaration = self.structs[id].clone();
            if declaration.semantic_fields().is_empty() {
                self.error(
                    declaration.span,
                    format!(
                        "`@CLayout` struct `{}` must declare at least one field",
                        declaration.name
                    ),
                );
                continue;
            }
            let mut visiting = HashSet::new();
            for field in declaration.semantic_fields() {
                let path = vec![declaration.name.clone(), field.name.clone()];
                if let Err(error) =
                    self.classify_c_ffi_type(field.ty, &[], false, path, &mut visiting)
                {
                    self.error(
                        declaration.span,
                        format!("`@CLayout` field is not C-FFI-safe: {}", error.render()),
                    );
                }
            }
        }

        // Generic applications may be inferred in bodies and therefore have
        // no dedicated declaration node. Validate every concrete canonical
        // application; the source declaration span is a deterministic
        // fallback when the application was synthesized by substitution.
        let concrete_layouts: Vec<_> = self
            .types
            .iter()
            .filter_map(|(ty, value)| match value {
                hir::Type::Struct(application) => {
                    let application = &self.struct_applications[*application];
                    let id = self.source_struct_id(application.template)?;
                    (self.structs[id].attributes.c_layout.is_some()
                        && !application.arguments.is_empty()
                        && !self.type_contains_param(ty))
                    .then_some((ty, application.template))
                }
                _ => None,
            })
            .collect();
        for (ty, id) in concrete_layouts {
            self.current_file = self.struct_files[&self.struct_id(id)];
            let mut visiting = HashSet::new();
            if let Err(error) =
                self.classify_c_ffi_type(ty, &[], false, vec![self.type_name(ty)], &mut visiting)
            {
                self.error(
                    self.structs[self.struct_id(id)].span,
                    format!(
                        "concrete `@CLayout` type is not C-FFI-safe: {}",
                        error.render()
                    ),
                );
            }
        }

        let fun_ptr_uses = self.fun_ptr_type_uses.clone();
        for (ty, file, span) in fun_ptr_uses {
            self.current_file = file;
            let mut visiting = HashSet::new();
            if let Err(error) =
                self.classify_c_ffi_type(ty, &[], false, vec![self.type_name(ty)], &mut visiting)
            {
                self.error(
                    span,
                    format!("`FunPtr` signature is not C-FFI-safe: {}", error.render()),
                );
            }
        }
    }
}
