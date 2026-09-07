use super::*;

impl Lowerer {
    pub(crate) fn require_core_enum(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
    ) -> Option<EnumId> {
        let candidate = self.enums_by_name.get(name).copied();
        if let Some(id) = candidate
            && self
                .enum_files
                .get(&id)
                .copied()
                .unwrap_or(self.user_file_index)
                < self.user_file_index
        {
            return Some(id);
        }
        self.current_file = 0;
        self.error(
            files[0].span,
            format!("scoop.core must define exactly one `{name}` enum"),
        );
        None
    }

    pub(crate) fn validate_unit_enum(&mut self, id: EnumId, names: &[&str]) {
        self.current_file = self.enum_files[&id];
        let declaration = &self.enums[id];
        let valid = declaration.type_params.is_empty()
            && declaration.interfaces.is_empty()
            && declaration.variants.len() == names.len()
            && declaration
                .variants
                .iter()
                .zip(names)
                .all(|(variant, name)| variant.name == *name && variant.fields.is_empty());
        if !valid {
            self.error(
                declaration.span,
                format!(
                    "core `{}` must declare unit variants `{}` in order",
                    declaration.name,
                    names.join("`, `")
                ),
            );
        }
    }

    pub(crate) fn interned_enum_type(&self, enum_id: EnumId) -> TypeId {
        self.enum_applications[self.enums[enum_id].self_application].canonical_type
    }

    pub(crate) fn validate_ffi_handle_struct(&mut self, id: StructId, name: &str) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind() == hir::TypeParamKind::Ref
            && matches!(declaration.semantic_fields(), [field] if field.name == "raw" && field.ty == self.integer_type(hir::IntegerKind::UNSIGNED_64))
            && declaration.interfaces.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                format!("core `{name}` must be `struct {name}<T : ref>(val raw: ULong)`"),
            );
        }
    }

    pub(crate) fn validate_gc_intrinsic(&mut self, id: FunctionId, kind: GcIntrinsic) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let signature = &self.signatures[&id];
        let parameter_matches = match kind {
            GcIntrinsic::Pin | GcIntrinsic::GetHandle => {
                matches!(signature.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                    && signature.return_ty == self.integer_type(hir::IntegerKind::UNSIGNED_64)
            }
            GcIntrinsic::Unpin | GcIntrinsic::ReleaseHandle => {
                matches!(signature.params.as_slice(), [param] if param.ty == self.integer_type(hir::IntegerKind::UNSIGNED_64))
                    && self.is_type_param(signature.return_ty, 0)
            }
        };
        let valid = !signature.is_suspend
            && signature.type_params.len() == 1
            && signature.type_params[0].kind() == hir::TypeParamKind::Ref
            && signature.attributes.safety == hir::Safety::Unsafe
            && signature.attributes.gc_effect == hir::GcEffect::Managed
            && function.method.is_none()
            && parameter_matches;
        if !valid {
            self.error(
                function.span,
                format!(
                    "intrinsic `{}` has an invalid core GC primitive signature",
                    kind.name()
                ),
            );
        }
    }

    pub(crate) fn validate_ptr_struct(&mut self, id: StructId) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind() == hir::TypeParamKind::Value
            && declaration.semantic_fields().is_empty()
            && declaration.constructors.is_empty()
            && declaration.interfaces.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                "core `Ptr` must be a fieldless intrinsic `struct Ptr<T : value>`".to_string(),
            );
        } else {
            let type_param = declaration.type_params[0].id;
            self.structs[id].gc_free_pointee_requirements =
                vec![hir::RequiresGcFreePointee { type_param }];
        }
    }

    pub(crate) fn validate_fun_ptr_struct(&mut self, id: StructId) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind() == hir::TypeParamKind::Any
            && declaration.semantic_fields().is_empty()
            && declaration.constructors.is_empty()
            && declaration.interfaces.is_empty()
            && declaration.methods.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                "core `FunPtr` must be a fieldless intrinsic `struct FunPtr<F>`".to_string(),
            );
        }
    }

    pub(crate) fn validate_ptr_method_intrinsic(
        &mut self,
        id: FunctionId,
        ptr: StructId,
        kind: hir::PointerIntrinsic,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let owner_matches = self.function_owner.get(&id) == Some(&Owner::Struct(ptr));
        let base = owner_matches
            && !sig.is_suspend
            && sig.owner_type_param_count == 1
            && sig.type_params.first().is_some_and(|param| {
                param.kind() == hir::TypeParamKind::Value && param.name == "T"
            })
            && function.attributes.safety == hir::Safety::Unsafe
            && function.attributes.gc_effect == hir::GcEffect::NoGc;
        let valid = base
            && match kind {
                hir::PointerIntrinsic::ToULong => {
                    function.name.ends_with(".toULong")
                        && sig.type_params.len() == 1
                        && sig.params.is_empty()
                        && sig.return_ty == self.integer_type(hir::IntegerKind::UNSIGNED_64)
                }
                hir::PointerIntrinsic::Cast => {
                    function.name.ends_with(".cast")
                        && sig.type_params.len() == 2
                        && sig.type_params[1].kind() == hir::TypeParamKind::Value
                        && sig.params.is_empty()
                        && self.is_ptr_param(sig.return_ty, 1)
                }
                hir::PointerIntrinsic::Load => {
                    function.name.ends_with(".load")
                        && sig.type_params.len() == 1
                        && sig.params.is_empty()
                        && self.is_type_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::LoadOffset => {
                    function.name.ends_with(".load")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if param.ty == self.integer_type(hir::IntegerKind::SIGNED_64))
                        && self.is_type_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::Store => {
                    function.name.ends_with(".store")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                        && sig.return_ty == self.unit
                }
                hir::PointerIntrinsic::StoreOffset => {
                    function.name.ends_with(".store")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [offset, value] if offset.ty == self.integer_type(hir::IntegerKind::SIGNED_64) && self.is_type_param(value.ty, 0))
                        && sig.return_ty == self.unit
                }
                hir::PointerIntrinsic::Plus | hir::PointerIntrinsic::Minus => {
                    sig.modifiers.operator
                        == Some(if kind == hir::PointerIntrinsic::Plus {
                            hir::OperatorKind::Plus
                        } else {
                            hir::OperatorKind::Minus
                        })
                        && !sig.modifiers.is_infix
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if param.ty == self.integer_type(hir::IntegerKind::SIGNED_64))
                        && self.is_ptr_param(sig.return_ty, 0)
                }
                _ => false,
            };
        if !valid {
            let intrinsic = match &function.kind {
                FunctionKind::Intrinsic(intrinsic) => intrinsic.kind.name(),
                FunctionKind::User(_) => "pointer".to_string(),
                FunctionKind::DerivedEquality => "derived equality".to_string(),
                FunctionKind::Extern(_) => "extern".to_string(),
            };
            self.error(
                function.span,
                format!("malformed core pointer intrinsic `{intrinsic}`"),
            );
        }
    }

    pub(crate) fn validate_pointer_top_level_intrinsic(
        &mut self,
        id: FunctionId,
        kind: hir::PointerIntrinsic,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let one_value_param = sig.type_params.len() == 1
            && sig.type_params[0].kind() == hir::TypeParamKind::Value
            && sig.owner_type_param_count == 0;
        let valid = function.method.is_none()
            && !sig.is_suspend
            && one_value_param
            && match kind {
                hir::PointerIntrinsic::AddressOf => {
                    function.name == "addressOf"
                        && function.attributes.safety == hir::Safety::Unsafe
                        && function.attributes.gc_effect == hir::GcEffect::Managed
                        && matches!(sig.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                        && self.is_ptr_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::SizeOf | hir::PointerIntrinsic::AlignOf => {
                    function.name
                        == if kind == hir::PointerIntrinsic::SizeOf {
                            "sizeOf"
                        } else {
                            "alignOf"
                        }
                        && function.attributes.safety == hir::Safety::Safe
                        && function.attributes.gc_effect == hir::GcEffect::NoGc
                        && sig.params.is_empty()
                        && sig.return_ty == self.integer_type(hir::IntegerKind::UNSIGNED_64)
                }
                _ => false,
            };
        if !valid {
            let intrinsic = match &function.kind {
                FunctionKind::Intrinsic(intrinsic) => intrinsic.kind.name(),
                FunctionKind::User(_) => "pointer".to_string(),
                FunctionKind::DerivedEquality => "derived equality".to_string(),
                FunctionKind::Extern(_) => "extern".to_string(),
            };
            self.error(
                function.span,
                format!("malformed core pointer intrinsic `{intrinsic}`"),
            );
        }
    }

    pub(crate) fn validate_pointer_type_uses(&mut self) {
        let uses = self.pointer_type_uses.clone();
        for (ty, file, span) in uses {
            let Type::Ptr(pointee) = self.types[ty] else {
                continue;
            };
            if self.type_contains_param(pointee) {
                continue;
            }
            if !self.is_gc_free(pointee) {
                self.current_file = file;
                self.error(
                    span,
                    format!(
                        "`Ptr` pointee must be GC-free, found {}",
                        self.type_name(pointee)
                    ),
                );
            }
        }
    }
}
