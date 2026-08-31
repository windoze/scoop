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
}
