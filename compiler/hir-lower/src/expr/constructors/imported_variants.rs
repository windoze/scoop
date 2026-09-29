//! Dependency variants construct ordinary enum values from actual declarations.

use super::*;
use hir::ImportedCallableSource;
use scoop_identity::CallableTemplateOrigin;

impl Lowerer {
    pub(in crate::expr) fn contextual_imported_variant(
        &self,
        name: &str,
        expected: Option<TypeId>,
    ) -> Option<(TypeId, usize)> {
        let owner = expected?;
        self.find_imported_variant(owner, name)
            .map(|index| (owner, index))
    }

    pub(in crate::expr) fn find_imported_variant(
        &self,
        owner: TypeId,
        name: &str,
    ) -> Option<usize> {
        let Type::ImportedEnum(enumeration) = &self.types[owner] else {
            return None;
        };
        enumeration
            .variants
            .iter()
            .position(|variant| variant.name == name)
    }

    pub(in crate::expr) fn lower_imported_variant_binding(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let hir::ImportedTarget::EnumVariant(variant) = binding.target() else {
            unreachable!("a dependency variant binding retains its typed variant identity")
        };
        if let Some(owner) = expected
            && let Type::ImportedEnum(enumeration) = &self.types[owner]
            && let Some(index) = enumeration
                .variants
                .iter()
                .position(|value| value.identity == variant.persistent())
        {
            return self.lower_imported_unit_variant(owner, index, name);
        }
        let candidate = match self
            .dependencies
            .as_ref()
            .expect("dependency declarations exist")
            .callable_declaration(CallableTemplateOrigin::VariantConstructor(
                variant.persistent(),
            )) {
            Ok(candidate) => candidate,
            Err(error) => {
                self.error(name.span, format!("invalid dependency variant: {error}"));
                return None;
            }
        };
        let owner = match self.imported_signature_type(candidate.interface().result()) {
            Ok(owner) => owner,
            Err(_) => {
                self.error(
                    name.span,
                    format!("cannot infer the enum type arguments for variant `{}`; provide an expected enum type", name.text),
                );
                return None;
            }
        };
        let Type::ImportedEnum(enumeration) = &self.types[owner] else {
            unreachable!("the shared variant declaration has its enum result type")
        };
        let index = enumeration
            .variants
            .iter()
            .position(|value| value.identity == variant.persistent())
            .expect("the shared enum contains its declared variant");
        self.lower_imported_unit_variant(owner, index, name)
    }

    pub(in crate::expr) fn lower_imported_unit_variant(
        &mut self,
        owner: TypeId,
        index: usize,
        name: &ast::Ident,
    ) -> Option<hir::Expr> {
        let Type::ImportedEnum(enumeration) = &self.types[owner] else {
            unreachable!("the selected variant has a dependency enum owner")
        };
        let variant = &enumeration.variants[index];
        if variant.style != hir::EnumSourceVariantStyleV1::Unit {
            self.error(
                name.span,
                format!(
                    "variant `{}` of `{}` takes arguments; use `{}(...)` to construct it",
                    name.text,
                    enumeration.declaration.name(),
                    name.text,
                ),
            );
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::ImportedVariantConstruct {
                owner,
                variant: variant.identity,
                args: Vec::new(),
            },
            ty: owner,
            span: name.span,
            origin: self.expression_origin(name.span),
        })
    }

    pub(in crate::expr) fn lower_imported_variant_construct(
        &mut self,
        owner: TypeId,
        index: usize,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let Type::ImportedEnum(enumeration) = &self.types[owner] else {
            unreachable!("the selected variant has a dependency enum owner")
        };
        let declaration =
            CallableTemplateOrigin::VariantConstructor(enumeration.variants[index].identity);
        let candidate = match self
            .dependencies
            .as_ref()
            .expect("dependency declarations exist")
            .callable_declaration(declaration)
        {
            Ok(candidate) => candidate,
            Err(error) => {
                self.error(name.span, format!("invalid dependency variant: {error}"));
                return None;
            }
        };
        match self.probe_imported_value_constructor(candidate, name, call, expected) {
            Ok(probe) => self.commit_imported_dependency_callable(probe, sink),
            Err(failure) => {
                self.commit_layer_diagnostics(*failure);
                None
            }
        }
    }
}
