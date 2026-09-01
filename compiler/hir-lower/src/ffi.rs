//! M12 C-FFI-safe classification and `@CLayout` validation.

use std::collections::HashSet;

use scoop_hir as hir;

use crate::Lowerer;

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
                hir::GlobalStorage::Local { .. } => None,
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
                    && self.globals[*previous].mutable == self.globals[*id].mutable
                    && self.types_equal(self.globals[*previous].ty, self.globals[*id].ty);
                if !compatible {
                    self.current_file = self.global_files[id];
                    self.error(
                        self.globals[*id].span,
                        format!(
                            "extern data symbol `{symbol}` conflicts with global `{}`",
                            self.globals[*previous].name
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
                self.current_file = self.global_files[id];
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
            if declaration.fields.is_empty() {
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
            for field in declaration.fields {
                let path = vec![declaration.name.clone(), field.name];
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
                hir::Type::Struct(id, args)
                    if self.structs[*id].attributes.c_layout.is_some()
                        && !args.is_empty()
                        && !self.type_contains_param(ty) =>
                {
                    Some((ty, *id))
                }
                _ => None,
            })
            .collect();
        for (ty, id) in concrete_layouts {
            self.current_file = self.struct_files[&id];
            let mut visiting = HashSet::new();
            if let Err(error) =
                self.classify_c_ffi_type(ty, &[], false, vec![self.type_name(ty)], &mut visiting)
            {
                self.error(
                    self.structs[id].span,
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

    fn classify_c_ffi_type(
        &mut self,
        ty: hir::TypeId,
        substitution: &[hir::TypeId],
        allow_unit: bool,
        path: Vec<String>,
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Result<Classification, CAbiError> {
        let resolved = match self.types[ty] {
            hir::Type::Param(index) => match substitution.get(index.into_raw() as usize) {
                Some(&argument) => argument,
                None => return Ok(Classification::Deferred),
            },
            _ => ty,
        };
        match self.types[resolved].clone() {
            hir::Type::Unit if allow_unit => Ok(Classification::Safe),
            hir::Type::Unit => Err(CAbiError {
                path,
                reason: "`Unit` is only allowed as a C ABI return type".to_string(),
            }),
            hir::Type::Int | hir::Type::UInt | hir::Type::Boolean | hir::Type::Ptr(_) => {
                Ok(Classification::Safe)
            }
            hir::Type::FunPtr(signature) => {
                let function = self.function_types[signature].clone();
                let mut deferred = false;
                for (index, parameter) in function.parameter_types.into_iter().enumerate() {
                    let mut parameter_path = path.clone();
                    parameter_path.push(format!("parameter{}", index + 1));
                    deferred |= self.classify_c_ffi_type(
                        parameter,
                        substitution,
                        false,
                        parameter_path,
                        visiting,
                    )? == Classification::Deferred;
                }
                let mut return_path = path;
                return_path.push("return".to_string());
                deferred |= self.classify_c_ffi_type(
                    function.return_type,
                    substitution,
                    true,
                    return_path,
                    visiting,
                )? == Classification::Deferred;
                Ok(if deferred {
                    Classification::Deferred
                } else {
                    Classification::Safe
                })
            }
            hir::Type::Struct(id, args) => {
                if self.structs[id].attributes.c_layout.is_none() {
                    return Err(CAbiError {
                        path,
                        reason: format!(
                            "ordinary struct `{}` has no stable C layout",
                            self.structs[id].name
                        ),
                    });
                }
                if !visiting.insert(resolved) {
                    return Err(CAbiError {
                        path,
                        reason: "recursive by-value C layout is not finite".to_string(),
                    });
                }
                let fields = self.structs[id].fields.clone();
                let mut deferred = false;
                for field in fields {
                    let field_ty = self.instantiate_ty(field.ty, &args);
                    let mut field_path = path.clone();
                    field_path.push(field.name);
                    deferred |= self.classify_c_ffi_type(
                        field_ty,
                        &[],
                        false,
                        field_path,
                        visiting,
                    )? == Classification::Deferred;
                }
                visiting.remove(&resolved);
                Ok(if deferred {
                    Classification::Deferred
                } else {
                    Classification::Safe
                })
            }
            hir::Type::Enum(id, args)
                if Some(id) == self.option_enum && args.len() == 1 =>
            {
                match self.types[args[0]] {
                    hir::Type::Ptr(_) | hir::Type::FunPtr(_) => self.classify_c_ffi_type(
                        args[0],
                        substitution,
                        false,
                        path,
                        visiting,
                    ),
                    _ => Err(CAbiError {
                        path,
                        reason: "only `Option<Ptr<T>>` and `Option<FunPtr<F>>` have a C ABI representation"
                            .to_string(),
                    }),
                }
            }
            hir::Type::Param(_) => Ok(Classification::Deferred),
            hir::Type::String
            | hir::Type::Class(_)
            | hir::Type::Interface(_, _)
            | hir::Type::Any
            | hir::Type::Array(_)
            | hir::Type::MutableArray(_)
            | hir::Type::Function(_) => Err(CAbiError {
                path,
                reason: format!("ref type `{}` is managed", self.type_name(resolved)),
            }),
            hir::Type::Tuple(_) => Err(CAbiError {
                path,
                reason: "tuple types have no stable C layout".to_string(),
            }),
            hir::Type::Enum(_, _) => Err(CAbiError {
                path,
                reason: "enum types have no M12 C ABI representation".to_string(),
            }),
        }
    }

    fn classify_scoop_abi_type(
        &mut self,
        ty: hir::TypeId,
        allow_unit: bool,
        path: Vec<String>,
    ) -> Result<(), CAbiError> {
        match self.types[ty].clone() {
            hir::Type::Unit if allow_unit => Ok(()),
            hir::Type::Unit => Err(CAbiError {
                path,
                reason: "`Unit` is only allowed as a Scoop ABI return type".to_string(),
            }),
            hir::Type::String
            | hir::Type::Class(_)
            | hir::Type::Interface(_, _)
            | hir::Type::Any
            | hir::Type::Array(_)
            | hir::Type::MutableArray(_)
            | hir::Type::Function(_) => Ok(()),
            hir::Type::Param(_) => Err(CAbiError {
                path,
                reason: "an extern signature must be fully concrete".to_string(),
            }),
            _ if self.is_gc_free(ty) => Ok(()),
            _ => Err(CAbiError {
                path,
                reason: format!(
                    "value aggregate `{}` contains a managed reference and has no M12 native-root layout",
                    self.type_name(ty)
                ),
            }),
        }
    }
}
