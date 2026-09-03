use super::*;

impl Lowerer {
    pub(crate) fn validate_source_location_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::SourceLocationCore> {
        let location = self.require_core_struct("SourceLocation", files);
        let current =
            self.require_intrinsic(hir::IntrinsicFunctionKind::CurrentSourceLocation, files);
        let (Some(location), Some(current)) = (location, current) else {
            return None;
        };

        self.current_file = self.struct_files[&location];
        let declaration = &self.structs[location];
        let location_span = declaration.span;
        let location_application = declaration.self_application;
        let fields = declaration.semantic_fields();
        let valid_location = declaration.type_params.is_empty()
            && declaration.attributes == hir::StructAttributes::default()
            && declaration.interfaces.is_empty()
            && declaration.methods.is_empty()
            && matches!(
                fields,
                [file, line, column, function_name, type_name]
                    if file.name == "file" && file.ty == self.string
                        && line.name == "line" && line.ty == self.int
                        && column.name == "column" && column.ty == self.int
                        && function_name.name == "functionName" && function_name.ty == self.string
                        && type_name.name == "typeName" && type_name.ty == self.string
            );
        if !valid_location {
            self.error(
                location_span,
                "core `SourceLocation` must declare `(file: String, line: Int, column: Int, functionName: String, typeName: String)`"
                    .to_string(),
            );
        }

        self.current_file = self.function_files[&current];
        let function = &self.functions[current];
        let signature = &self.signatures[&current];
        let location_type = self.struct_applications[location_application].canonical_type;
        let valid_current = function.name == "getCurrentSourceLocation"
            && function.method.is_none()
            && matches!(function.genericity, hir::FunctionGenericity::Plain)
            && !signature.is_suspend
            && signature.params.is_empty()
            && signature.return_ty == location_type
            && signature.attributes == hir::FunctionAttributes::default();
        if !valid_current {
            self.error(
                function.span,
                "intrinsic `current_source_location` must be `fun getCurrentSourceLocation(): SourceLocation`"
                    .to_string(),
            );
        }

        Some(hir::SourceLocationCore { location, current })
    }
}
