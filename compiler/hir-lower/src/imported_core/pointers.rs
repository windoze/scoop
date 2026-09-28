//! Source spelling resolves through the imported core role before producing
//! the existing structural pointer HIR type.

use super::*;

impl Lowerer {
    pub(crate) fn resolve_imported_generic_type_target(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &scoop_ast::Ident,
        arguments: &[scoop_ast::TypeRef],
    ) -> Option<hir::TypeId> {
        let hir::ImportedTarget::GenericType(declaration) = binding.target() else {
            return self.resolve_imported_dependency_type_target(binding, name, true);
        };
        let CoreLoweringAuthority::Imported(core) = &self.core else {
            return self.resolve_imported_nominal_type_arguments(binding, name, arguments);
        };
        let role = if declaration.persistent() == core.fundamental_types().ptr().persistent() {
            hir::IntrinsicTypeKind::Ptr
        } else if declaration.persistent() == core.fundamental_types().fun_ptr().persistent() {
            hir::IntrinsicTypeKind::FunPtr
        } else {
            return self.resolve_imported_nominal_type_arguments(binding, name, arguments);
        };
        let [argument] = arguments else {
            self.error(
                name.span,
                format!(
                    "type `{}` takes 1 type argument, but {} were supplied",
                    name.text,
                    arguments.len()
                ),
            );
            return None;
        };
        let argument = self.resolve_type_ref(argument)?;
        let ty = if role == hir::IntrinsicTypeKind::Ptr {
            let ty = self.intern_type(hir::Type::Ptr(argument));
            self.pointer_type_uses
                .push((ty, self.current_file, name.span));
            ty
        } else {
            let hir::Type::Function(function) = self.types[argument] else {
                self.error(
                    name.span,
                    "`FunPtr` type argument must be an ordinary concrete function type".into(),
                );
                return None;
            };
            if self.function_types[function].is_suspend
                || self.function_type_contains_param(function)
            {
                self.error(
                    name.span,
                    "`FunPtr` type argument must be an ordinary concrete function type".into(),
                );
                return None;
            }
            let ty = self.intern_type(hir::Type::FunPtr(function));
            self.fun_ptr_type_uses
                .push((ty, self.current_file, name.span));
            ty
        };
        self.retain_imported_alias_target_bindings(
            binding,
            hir::ExternalHirTargetV1::Nominal(
                scoop_identity::NominalDeclarationOwner::GenericTemplate(declaration.persistent()),
            ),
        );
        Some(ty)
    }
}
