use super::*;
use hir::{AnnotationCallableV1 as Callable, AnnotationTargetV1 as A};
use scoop_identity::{
    BindableEntity as B, CallableTemplateOrigin as C, NominalDeclarationOwner as N,
};

impl Projection<'_> {
    pub(super) fn bindings(&self) -> Result<Vec<ExportBinding>> {
        self.export
            .export_binding_identities
            .iter()
            .map(|binding| {
                let key = binding.key();
                let target = match key.target() {
                    B::Type(id) => A::Nominal(N::Concrete(id)),
                    B::GenericType(id) => A::Nominal(N::GenericTemplate(id)),
                    B::Function(id) => A::Callable(Callable::Declaration(C::Function(id))),
                    B::GenericFunction(id) => {
                        A::Callable(Callable::Declaration(C::GenericFunction(id)))
                    }
                    B::Property(id) => A::Property(id),
                    B::ExtensionProperty(id) => A::ExtensionProperty(id),
                    B::TypeAlias(id) => A::TypeAlias(id),
                    B::EnumVariant(id) => A::Variant(id),
                    B::Annotation(id) => A::Annotation(id),
                    B::ObjectValue(id) => {
                        return self.binding(binding, SymbolKey::object_value(id)?);
                    }
                };
                self.binding(binding, SymbolKey::declaration(target)?)
            })
            .collect()
    }

    fn binding(
        &self,
        binding: &hir::HirExportBindingIdentity,
        target: SymbolKey,
    ) -> Result<ExportBinding> {
        let key = binding.key();
        let source = self
            .export
            .public_export_bindings
            .get(binding.id())
            .expect("each public binding has its checked source");
        Ok(ExportBinding {
            package: key
                .package()
                .segments()
                .iter()
                .map(|part| part.as_str().into())
                .collect(),
            name: key.name().as_str().into(),
            target,
            reexport: matches!(source.source(), hir::ExportBindingSourceV1::Reexport { .. }),
        })
    }
}
