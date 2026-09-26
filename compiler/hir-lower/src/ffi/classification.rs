use super::*;

#[derive(Default)]
struct SignatureValidation {
    visiting: HashSet<hir::FunctionTypeId>,
    safe: HashSet<hir::FunctionTypeId>,
}

impl Lowerer {
    pub(super) fn classify_c_ffi_type(
        &mut self,
        ty: hir::TypeId,
        substitution: &[hir::TypeId],
        allow_unit: bool,
        path: Vec<String>,
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Result<Classification, CAbiError> {
        self.classify_c_ffi_type_inner(
            ty,
            substitution,
            allow_unit,
            path,
            visiting,
            &mut SignatureValidation::default(),
        )
    }

    fn classify_c_ffi_type_inner(
        &mut self,
        ty: hir::TypeId,
        substitution: &[hir::TypeId],
        allow_unit: bool,
        path: Vec<String>,
        visiting: &mut HashSet<hir::TypeId>,
        signatures: &mut SignatureValidation,
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
            hir::Type::Integer(_) | hir::Type::Boolean => Ok(Classification::Safe),
            hir::Type::Ptr(pointee) => {
                self.classify_c_pointer_pointee(pointee, substitution, path, visiting, signatures)
            }
            hir::Type::FunPtr(signature) => {
                if signatures.safe.contains(&signature) || !signatures.visiting.insert(signature) {
                    return Ok(Classification::Safe);
                }
                let function = self.function_types[signature].clone();
                let result = (|| {
                    let mut deferred = false;
                    for (index, parameter) in function.parameter_types.into_iter().enumerate() {
                        let mut parameter_path = path.clone();
                        parameter_path.push(format!("parameter{}", index + 1));
                        deferred |= self.classify_c_ffi_type_inner(
                            parameter,
                            substitution,
                            false,
                            parameter_path,
                            &mut HashSet::new(),
                            signatures,
                        )? == Classification::Deferred;
                    }
                    let mut return_path = path;
                    return_path.push("return".to_string());
                    deferred |= self.classify_c_ffi_type_inner(
                        function.return_type,
                        substitution,
                        true,
                        return_path,
                        &mut HashSet::new(),
                        signatures,
                    )? == Classification::Deferred;
                    Ok(if deferred {
                        Classification::Deferred
                    } else {
                        Classification::Safe
                    })
                })();
                signatures.visiting.remove(&signature);
                if matches!(result, Ok(Classification::Safe)) {
                    signatures.safe.insert(signature);
                }
                result
            }
            hir::Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let id = application.template;
                let scalar_projection =
                    Some(id) == self.ffi_pinned_ptr || Some(id) == self.ffi_gc_handle;
                if self.structs[id].attributes.c_layout.is_none() && !scalar_projection {
                    return Err(CAbiError {
                        path,
                        reason: format!(
                            "ordinary struct `{}` has no stable C layout",
                            self.structs[id].name
                        ),
                    });
                }
                let fields = self.structs[id].semantic_fields().to_vec();
                let fields = fields
                    .into_iter()
                    .map(|field| {
                        (
                            field.name,
                            self.instantiate_ty(field.ty, &application.arguments),
                        )
                    })
                    .collect();
                self.classify_c_struct_fields(resolved, fields, path, visiting, signatures)
            }
            hir::Type::ImportedStruct(structure) => {
                let hir::NominalSourceShapeV1::Struct(shape) =
                    structure.declaration.interface.source_shape()
                else {
                    unreachable!("an imported struct retains a struct declaration")
                };
                if matches!(
                    shape.c_layout_policy(),
                    hir::NominalCLayoutPolicyV1::Ordinary
                ) && !matches!(
                    structure.declaration.c_abi,
                    hir::NativeBoundaryCAbiV1::UInt64Field { .. }
                ) {
                    return Err(CAbiError {
                        path,
                        reason: format!(
                            "ordinary struct `{}` has no stable C layout",
                            structure.declaration.name()
                        ),
                    });
                }
                let fields = structure
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty))
                    .collect();
                self.classify_c_struct_fields(resolved, fields, path, visiting, signatures)
            }
            hir::Type::ImportedEnum(_) => Err(CAbiError {
                path,
                reason: "enum types have no M12 C ABI representation".to_string(),
            }),
            hir::Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                if Some(application.template) != self.option_enumeration()
                    || application.arguments.len() != 1
                {
                    return Err(CAbiError {
                        path,
                        reason: "enum types have no M12 C ABI representation".to_string(),
                    });
                }
                match self.types[application.arguments[0]] {
                    hir::Type::Ptr(_) | hir::Type::FunPtr(_) => self.classify_c_ffi_type_inner(
                        application.arguments[0],
                        substitution,
                        false,
                        path,
                        visiting,
                        signatures,
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
            | hir::Type::ImportedClass(_)
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

    fn classify_c_pointer_pointee(
        &mut self,
        pointee: hir::TypeId,
        substitution: &[hir::TypeId],
        path: Vec<String>,
        visiting: &mut HashSet<hir::TypeId>,
        signatures: &mut SignatureValidation,
    ) -> Result<Classification, CAbiError> {
        let pointee = match self.types[pointee] {
            hir::Type::Param(parameter) => match substitution.get(parameter.into_raw() as usize) {
                Some(&argument) => argument,
                None => return Ok(Classification::Deferred),
            },
            _ => pointee,
        };
        if matches!(self.types[pointee], hir::Type::Unit) {
            return Ok(Classification::Safe);
        }
        match self.is_zero_sized_type(pointee, substitution, &mut HashSet::new()) {
            Some(true) => {
                return Err(CAbiError {
                    path,
                    reason: format!(
                        "pointer pointee `{}` is zero-sized; only `Ptr<Unit>` maps to `void *`",
                        self.type_name(pointee)
                    ),
                });
            }
            Some(false) => {}
            None => return Ok(Classification::Deferred),
        }

        if let hir::Type::Struct(application) = self.types[pointee] {
            let application = &self.struct_applications[application];
            if self.structs[application.template]
                .attributes
                .c_layout
                .is_some()
            {
                // A pointer edge names the refined C-layout object instead of
                // recursively embedding its fields. This permits ordinary C
                // self-reference while by-value cycles remain rejected.
                return Ok(Classification::Safe);
            }
        }
        self.classify_c_ffi_type_inner(pointee, substitution, false, path, visiting, signatures)
    }

    fn classify_c_struct_fields(
        &mut self,
        ty: hir::TypeId,
        fields: Vec<(String, hir::TypeId)>,
        path: Vec<String>,
        visiting: &mut HashSet<hir::TypeId>,
        signatures: &mut SignatureValidation,
    ) -> Result<Classification, CAbiError> {
        if !visiting.insert(ty) {
            return Err(CAbiError {
                path,
                reason: "recursive by-value C layout is not finite".to_owned(),
            });
        }
        let mut deferred = false;
        for (name, field) in fields {
            let mut field_path = path.clone();
            field_path.push(name);
            deferred |= self.classify_c_ffi_type_inner(
                field,
                &[],
                false,
                field_path,
                visiting,
                signatures,
            )? == Classification::Deferred;
        }
        visiting.remove(&ty);
        Ok(if deferred {
            Classification::Deferred
        } else {
            Classification::Safe
        })
    }

    fn is_zero_sized_type(
        &mut self,
        ty: hir::TypeId,
        substitution: &[hir::TypeId],
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Option<bool> {
        let ty = match self.types[ty] {
            hir::Type::Param(parameter) => *substitution.get(parameter.into_raw() as usize)?,
            _ => ty,
        };
        match self.types[ty].clone() {
            hir::Type::ImportedStruct(structure) => {
                for field in &structure.fields {
                    if !self.is_zero_sized_type(field.ty, &[], visiting)? {
                        return Some(false);
                    }
                }
                Some(true)
            }

            hir::Type::Unit => Some(true),
            hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::String
            | hir::Type::Class(_)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Function(_)
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_)
            | hir::Type::Enum(_)
            | hir::Type::ImportedEnum(_)
            | hir::Type::ImportedClass(_) => Some(false),
            hir::Type::Param(_) => None,
            hir::Type::Tuple(elements) => {
                for element in elements {
                    if !self.is_zero_sized_type(element, substitution, visiting)? {
                        return Some(false);
                    }
                }
                Some(true)
            }
            hir::Type::Struct(application) => {
                if !visiting.insert(ty) {
                    return None;
                }
                let application = self.struct_applications[application].clone();
                let fields = self.structs[application.template]
                    .semantic_fields()
                    .to_vec();
                for field in fields {
                    let field_ty = self.instantiate_ty(field.ty, &application.arguments);
                    if !self.is_zero_sized_type(field_ty, substitution, visiting)? {
                        visiting.remove(&ty);
                        return Some(false);
                    }
                }
                visiting.remove(&ty);
                Some(true)
            }
        }
    }

    pub(super) fn classify_scoop_abi_type(
        &mut self,
        ty: hir::TypeId,
        allow_unit: bool,
        path: Vec<String>,
    ) -> Result<(), CAbiError> {
        if self.type_contains_param(ty) {
            return Err(CAbiError {
                path,
                reason: "an extern signature must be fully concrete".to_string(),
            });
        }
        match self.types[ty].clone() {
            hir::Type::Unit if allow_unit => Ok(()),
            hir::Type::Unit => Err(CAbiError {
                path,
                reason: "`Unit` is only allowed as a Scoop ABI return type".to_string(),
            }),
            hir::Type::Param(_) => Err(CAbiError {
                path,
                reason: "an extern signature must be fully concrete".to_string(),
            }),
            hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::String
            | hir::Type::Class(..)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Function(_)
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_)
            | hir::Type::ImportedStruct(_)
            | hir::Type::ImportedEnum(_)
            | hir::Type::ImportedClass(_)
            | hir::Type::Struct(_)
            | hir::Type::Enum(_)
            | hir::Type::Tuple(_) => Ok(()),
        }
    }
}
