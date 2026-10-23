use super::*;
use hir::{EditingCallee as C, ImportedCallableSource, SemanticConstructor as Constructor};
use scoop_identity::CallableTemplateOrigin;

mod parameters;

impl Lowerer {
    pub(crate) fn observe_variant_reference(
        &mut self,
        span: ast::Span,
        variant: hir::EnumVariantApplication,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let target = self.editing_variant_target(variant);
        self.observe_callee_reference(span, target.into());
    }

    pub(crate) fn editing_variant_target(
        &self,
        variant: hir::EnumVariantApplication,
    ) -> hir::SemanticVariant {
        self.enum_member_identities
            .as_ref()
            .and_then(|identities| identities.variant_declaration(variant.variant))
            .map_or(
                hir::SemanticVariant::Imported(variant.variant),
                hir::SemanticVariant::Current,
            )
    }

    pub(crate) fn editing_nominal_callee(
        source: crate::call_resolution::candidates::NominalConstructorSource,
    ) -> Option<C> {
        use crate::call_resolution::candidates::NominalConstructorSource as Source;
        Some(match source {
            Source::Class(constructor) => C::Constructor(Constructor::CurrentClass(constructor)),
            Source::Struct(constructor) => C::Constructor(Constructor::CurrentStruct(constructor)),
            Source::Variant(variant) => C::CurrentVariant(variant),
            Source::IntrinsicClass(_)
            | Source::ArrayGenerate(_)
            | Source::ImportedArray(_)
            | Source::ImportedArrayGenerate(_)
            | Source::Atomic(_)
            | Source::ImportedAtomic(_) => return None,
        })
    }

    pub(crate) fn editing_imported_construction_callee(
        origin: CallableTemplateOrigin,
    ) -> Option<C> {
        match origin {
            CallableTemplateOrigin::Constructor(constructor) => {
                Some(C::Constructor(Constructor::Imported(constructor)))
            }
            CallableTemplateOrigin::VariantConstructor(variant) => {
                Some(C::ImportedVariant(variant))
            }
            _ => None,
        }
    }

    pub(super) fn editing_callee_origin(&self, callee: C) -> Option<hir::SemanticSourceLocation> {
        let origin = match callee {
            C::Callable(callable) => return self.editing_callable_origin(callable),
            C::Constructor(Constructor::CurrentClass(constructor)) => {
                self.class_constructors[constructor].origin
            }
            C::Constructor(Constructor::CurrentStruct(constructor)) => {
                self.struct_constructors[constructor].origin
            }
            C::CurrentVariant(variant) => {
                return Some(hir::SemanticSourceLocation {
                    source: self
                        .source_identity(*self.enum_files.get(&variant.enumeration())?)
                        .clone(),
                    span: self.enum_variant_spans[&variant],
                });
            }
            C::Constructor(Constructor::Imported(constructor)) => {
                return self.editing_imported_callee_origin(CallableTemplateOrigin::Constructor(
                    constructor,
                ));
            }
            C::ImportedVariant(variant) => {
                return self.editing_imported_callee_origin(
                    CallableTemplateOrigin::VariantConstructor(variant),
                );
            }
        };
        Some(hir::SemanticSourceLocation {
            source: self.source_identity(origin.file as usize).clone(),
            span: origin.span,
        })
    }

    fn editing_imported_callee_origin(
        &self,
        origin: CallableTemplateOrigin,
    ) -> Option<hir::SemanticSourceLocation> {
        let declaration = self
            .dependencies
            .as_ref()?
            .callable_declaration(origin)
            .ok()?;
        crate::semantic_queries::imported_location(declaration.definition_origin().origin()).ok()
    }
}
