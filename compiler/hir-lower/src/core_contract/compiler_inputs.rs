use super::*;

impl Lowerer {
    pub(crate) fn validate_compile_time_input_intrinsics(&mut self) {
        for kind in [
            hir::IntrinsicFunctionKind::CompileEnv,
            hir::IntrinsicFunctionKind::IncludeStr,
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
                    _ => unreachable!("only String-parameter input intrinsics are checked here"),
                }
                && signature.attributes == hir::FunctionAttributes::default();
            if !valid {
                self.error(declaration.span, match kind {
                hir::IntrinsicFunctionKind::CompileEnv => "intrinsic `compile_env` must be an ordinary top-level `fun env(name: String): Option<String>`",
                hir::IntrinsicFunctionKind::IncludeStr => "intrinsic `include_str` must be an ordinary top-level `fun includeStr(path: String): String`",
                _ => unreachable!("only String-parameter input intrinsics are checked here"),
            }.into());
            }
        }
    }
}
