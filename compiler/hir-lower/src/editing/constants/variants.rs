use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

impl DeclarationAnalysis {
    pub fn static_variant_reference(
        &self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<V<Option<hir::EditingStaticVariantReference>>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        Ok(V::Known(
            self.static_variant_references
                .iter()
                .find(|reference| {
                    &reference.origin.source == source && reference.origin.span == span
                })
                .cloned(),
        ))
    }
}

impl Lowerer {
    pub(crate) fn observe_static_variant(
        &mut self,
        span: ast::Span,
        variant: hir::EnumVariantApplication,
        value_type: hir::TypeId,
    ) {
        let Some(name_span) = self
            .constant_frame()
            .and_then(|frame| frame.names.get(&span).copied())
        else {
            return;
        };
        let target = self.editing_variant_target(variant);
        let definition = self.editing_callee_origin(target.into());
        let template = self.enum_applications[variant.owner].template;
        let index = self.enum_variant_index(variant) as usize;
        let name = self.enum_definition(template).variants[index].name.clone();
        let reference = hir::EditingStaticVariantReference {
            origin: hir::SemanticSourceLocation {
                source: self.source_identity(self.current_file).clone(),
                span: name_span,
            },
            target,
            name,
            value_type,
            definition,
            imports: Vec::new(),
        };
        self.constant_frame()
            .expect("active constant initializer")
            .variants
            .insert(name_span, reference);
    }
}
