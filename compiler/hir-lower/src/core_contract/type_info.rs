use super::*;

impl Lowerer {
    pub(crate) fn validate_type_info_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<FunctionId> {
        const NAME: &str = "__scoopTypeReference";
        let reference = self
            .core_nominal_target("TypeRef")
            .and_then(|target| match target {
                NominalTarget::Class(id) if self.classes[id].type_params.is_empty() => {
                    Some(self.class_applications[self.classes[id].self_application].canonical_type)
                }
                _ => None,
            });
        let candidates = self
            .top_level
            .iter()
            .copied()
            .filter(|function| {
                self.source_function_declarations
                    .get(function)
                    .is_some_and(|declaration| declaration.name == NAME)
            })
            .collect::<Vec<_>>();
        let (Some(reference), [factory]) = (reference, candidates.as_slice()) else {
            self.current_file = self.core_diagnostic_file();
            self.error(files[self.current_file].span,
                "scoop.core must define TypeRef and one internal `fun <T> __scoopTypeReference(): TypeRef`".into());
            return None;
        };
        let factory = *factory;
        self.current_file = self.function_files[&factory];
        let declaration = &self.functions[factory];
        let signature = &self.signatures[&factory];
        if declaration.access.declared != hir::DeclaredVisibility::Internal
            || declaration.type_param_count() != 1
            || declaration.method.is_some()
            || signature.is_suspend
            || !signature.params.is_empty()
            || signature.return_ty != reference
            || signature.attributes != hir::FunctionAttributes::default()
            || !matches!(declaration.kind, hir::FunctionKind::User(_))
        {
            self.error(declaration.span,
                "core type reference factory must be internal, managed, non-suspend `fun <T> __scoopTypeReference(): TypeRef`".into());
        }
        for (kind, result) in [
            (hir::IntrinsicFunctionKind::AnnotationKey, "AnnotationKey"),
            (
                hir::IntrinsicFunctionKind::TypeReference,
                "TypeReferenceData",
            ),
            (
                hir::IntrinsicFunctionKind::TypeDescription,
                "TypeDescription",
            ),
        ] {
            let Some(&(function, _)) = self.intrinsic_functions.get(&kind) else {
                continue;
            };
            let Some(result) = self.require_core_struct(result, files) else {
                continue;
            };
            let result =
                self.struct_applications[self.structs[result].self_application].canonical_type;
            self.current_file = self.function_files[&function];
            let declaration = &self.functions[function];
            let signature = &self.signatures[&function];
            if declaration.type_param_count() != 1
                || declaration.method.is_some()
                || signature.is_suspend
                || !signature.params.is_empty()
                || signature.return_ty != result
                || signature.attributes != hir::FunctionAttributes::default()
            {
                self.error(declaration.span, format!("intrinsic `{}` must have one type parameter, no value parameters, and its core description result type", kind.name()));
            }
            self.set_source_context(hir::SourceContextSubject::Function(function));
        }
        Some(factory)
    }
}
