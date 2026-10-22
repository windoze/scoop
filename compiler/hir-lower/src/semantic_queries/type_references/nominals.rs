use super::*;

impl Lowerer {
    pub(super) fn apply_query_nominal_type(
        &mut self,
        declaration: hir::SourceNominalId,
        arguments: Vec<hir::TypeId>,
        span: Span,
    ) -> Option<hir::TypeId> {
        let parameters = self
            .nominal_declaration_parameters(declaration)
            .map_err(|error| self.error(span, error.diagnostic("queried nominal declaration")))
            .ok()?;
        if parameters.len() != arguments.len() {
            self.error(
                span,
                format!(
                    "queried type takes {} type argument(s), but {} were supplied",
                    parameters.len(),
                    arguments.len()
                ),
            );
            return None;
        }
        if !self.check_type_argument_kinds(&parameters, &arguments, span, "queried type") {
            return None;
        }
        if self.nominal_intrinsic_kind(declaration) == Some(hir::IntrinsicTypeKind::FunPtr) {
            let hir::Type::Function(function) = self.types[arguments[0]] else {
                self.error(
                    span,
                    "`FunPtr` type argument must be an ordinary concrete function type".into(),
                );
                return None;
            };
            if !self.validate_fun_ptr_signature(function, span) {
                return None;
            }
        }
        self.apply_nominal_type(declaration, arguments)
            .map_err(|error| self.error(span, error.diagnostic("queried type application")))
            .ok()
    }

    pub(super) fn query_pointer_declaration(&self, function: bool) -> hir::SourceNominalId {
        match &self.core {
            CoreLoweringAuthority::Defined => {
                let declaration = if function {
                    self.ffi_fun_ptr
                } else {
                    self.ffi_ptr
                }
                .expect("fixed queries follow core pointer declaration resolution");
                self.nominal_identity(Owner::Struct(declaration))
                    .declaration_id()
            }
            CoreLoweringAuthority::Imported(core) => hir::SourceNominalId::GenericTemplate(
                if function {
                    core.fundamental_types().fun_ptr()
                } else {
                    core.fundamental_types().ptr()
                }
                .persistent(),
            ),
        }
    }
}
