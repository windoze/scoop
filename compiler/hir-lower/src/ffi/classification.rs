use super::*;

impl Lowerer {
    pub(super) fn classify_c_ffi_type(
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
            hir::Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let id = application.template;
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
                let fields = self.structs[id].semantic_fields().to_vec();
                let mut deferred = false;
                for field in fields {
                    let field_ty = self.instantiate_ty(field.ty, &application.arguments);
                    let mut field_path = path.clone();
                    field_path.push(field.name);
                    deferred |=
                        self.classify_c_ffi_type(field_ty, &[], false, field_path, visiting)?
                            == Classification::Deferred;
                }
                visiting.remove(&resolved);
                Ok(if deferred {
                    Classification::Deferred
                } else {
                    Classification::Safe
                })
            }
            hir::Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                if Some(application.template) != self.option_enum
                    || application.arguments.len() != 1
                {
                    return Err(CAbiError {
                        path,
                        reason: "enum types have no M12 C ABI representation".to_string(),
                    });
                }
                match self.types[application.arguments[0]] {
                    hir::Type::Ptr(_) | hir::Type::FunPtr(_) => self.classify_c_ffi_type(
                        application.arguments[0],
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
            | hir::Type::Class(..)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Function(_) => Err(CAbiError {
                path,
                reason: format!("ref type `{}` is managed", self.type_name(resolved)),
            }),
            hir::Type::Tuple(_) => Err(CAbiError {
                path,
                reason: "tuple types have no stable C layout".to_string(),
            }),
        }
    }

    pub(super) fn classify_scoop_abi_type(
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
            | hir::Type::Class(..)
            | hir::Type::Interface(_)
            | hir::Type::Any
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
