use super::*;
use crate::NominalTarget;

impl Lowerer {
    pub(in crate::class::inheritance) fn selected_codec_value(
        &mut self,
        context: CodingContext,
        selected: &SelectedCodec,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let value = match &selected.value {
            CodecValue::Dependency(dependency) => self.coding_dependency_value(dependency, span)?,
            CodecValue::This => self
                .lower_current_this(span)
                .expect("the codec receiver is available"),
            CodecValue::Singleton(ty) => self.coding_singleton(*ty, span)?,
            CodecValue::Container { companion, element } => {
                let receiver = self.coding_singleton(*companion, span)?;
                let receiver = self.coding_local(receiver, span, sink);
                let element = self.selected_codec_value(context, element, span, sink)?;
                self.coding_call(
                    receiver,
                    context.direction.factory(),
                    vec![element],
                    span,
                    sink,
                )?
            }
            CodecValue::Tuple(elements) => {
                self.coding_tuple_function(context, selected.ty, elements, span, sink)?
            }
        };
        let interface = self
            .apply_nominal_type(context.interface, vec![selected.ty])
            .expect("a coding protocol has one unconstrained parameter");
        Some(self.adapt_to(value, interface))
    }

    fn coding_singleton(&mut self, ty: TypeId, span: Span) -> Option<hir::Expr> {
        let application = self
            .nominal_application(ty)
            .expect("a selected singleton has its complete application");
        if let Some(NominalTarget::Object(object)) = self.nominal_target_for_type(ty) {
            self.lower_singleton_application(object, application.arguments, span)
        } else {
            self.lower_imported_singleton_type(ty, span)
        }
    }
}
