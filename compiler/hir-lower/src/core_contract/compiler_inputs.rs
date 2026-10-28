use super::*;

impl Lowerer {
    pub(crate) fn validate_compile_time_input_intrinsics(&mut self) {
        self.validate_include_directory_intrinsic();
        for kind in [
            hir::IntrinsicFunctionKind::CompileEnv,
            hir::IntrinsicFunctionKind::IncludeStr,
            hir::IntrinsicFunctionKind::IncludeBytes,
        ] {
            let Some(&(function, _)) = self.intrinsic_functions.get(&kind) else {
                continue;
            };
            self.current_file = self.function_files[&function];
            let declaration = &self.functions[function];
            let signature = &self.signatures[&function];
            let valid = declaration.method.is_none()
                && matches!(declaration.genericity, hir::FunctionGenericity::Plain)
                && !signature.is_suspend
                && matches!(signature.params.as_slice(), [parameter]
                if parameter.ty == self.string && matches!(parameter.calling, crate::model::FnParamCalling::Required))
                && match kind {
                    hir::IntrinsicFunctionKind::CompileEnv => {
                        self.as_option(signature.return_ty) == Some(self.string)
                    }
                    hir::IntrinsicFunctionKind::IncludeStr => {
                        self.types_equal(signature.return_ty, self.string)
                    }
                    hir::IntrinsicFunctionKind::IncludeBytes => {
                        matches!(self.array_type_info(signature.return_ty), Some(array) if array.kind == crate::types::ArrayKind::Immutable && matches!(self.types[array.element], hir::Type::Integer(hir::IntegerKind::SIGNED_8)))
                    }
                    _ => unreachable!("only String-parameter input intrinsics are checked here"),
                }
                && signature.attributes == hir::FunctionAttributes::default();
            if !valid {
                self.error(declaration.span, match kind {
                hir::IntrinsicFunctionKind::CompileEnv => "intrinsic `compile_env` must be an ordinary top-level `fun env(name: String): Option<String>`",
                hir::IntrinsicFunctionKind::IncludeStr => "intrinsic `include_str` must be an ordinary top-level `fun includeStr(path: String): String`",
                hir::IntrinsicFunctionKind::IncludeBytes => "intrinsic `include_bytes` must be an ordinary top-level `fun includeBytes(path: String): Array<Byte>`",
                _ => unreachable!("only String-parameter input intrinsics are checked here"),
            }.into());
            }
        }
    }
}

impl Lowerer {
    fn validate_include_directory_intrinsic(&mut self) {
        let Some(&(function, _)) = self
            .intrinsic_functions
            .get(&hir::IntrinsicFunctionKind::IncludeDir)
        else {
            return;
        };
        self.current_file = self.function_files[&function];
        let declaration = &self.functions[function];
        let signature = self.signatures[&function].clone();
        let span = declaration.span;
        let valid = declaration.method.is_none()
            && matches!(declaration.genericity, hir::FunctionGenericity::Plain)
            && !signature.is_suspend
            && signature.attributes == hir::FunctionAttributes::default()
            && matches!(signature.params.as_slice(), [path, include, exclude]
            if path.ty == self.string
            && matches!(path.calling, crate::model::FnParamCalling::Required)
            && [include, exclude].iter().all(|parameter| {
                matches!(parameter.calling, crate::model::FnParamCalling::Default { .. })
                && self.array_type_info(parameter.ty).is_some_and(|array| {
                    array.kind == crate::types::ArrayKind::Immutable && array.element == self.string
                })
            }))
            && self.embedded_directory_types(signature.return_ty).is_some();
        if !valid {
            self.error(span, "intrinsic `include_dir` must be an ordinary top-level `fun includeDir(path: String, include: Array<String> = [\"**\"], exclude: Array<String> = []): EmbeddedDir`, with ordinary directory and file structs".into());
        }
    }

    /// Resolve the real result declarations; no nominal is looked up by name.
    pub(crate) fn embedded_directory_types(
        &mut self,
        ty: hir::TypeId,
    ) -> Option<(hir::StructApplicationId, hir::TypeId)> {
        let hir::Type::Struct(directory) = self.types[ty] else {
            return None;
        };
        let directory_template = self.struct_applications[directory].template;
        if self.struct_interior_mutable(directory_template)
            || self.struct_c_layout(directory_template).is_some()
        {
            return None;
        }
        let fields = self.struct_fields(ty)?.fields;
        let [files] = fields.as_slice() else {
            return None;
        };
        if files.name != "files" {
            return None;
        }
        let array = self.array_type_info(files.ty)?;
        if array.kind != crate::types::ArrayKind::Immutable {
            return None;
        }
        let hir::Type::Struct(file) = self.types[array.element] else {
            return None;
        };
        let file_template = self.struct_applications[file].template;
        if self.struct_interior_mutable(file_template)
            || self.struct_c_layout(file_template).is_some()
        {
            return None;
        }
        let fields = self.struct_fields(array.element)?.fields;
        let [path, bytes] = fields.as_slice() else {
            return None;
        };
        let bytes_array = self.array_type_info(bytes.ty)?;
        (path.name == "path"
            && path.ty == self.string
            && bytes.name == "bytes"
            && bytes_array.kind == crate::types::ArrayKind::Immutable
            && matches!(
                self.types[bytes_array.element],
                hir::Type::Integer(hir::IntegerKind::SIGNED_8)
            ))
        .then_some((directory, files.ty))
    }
}
