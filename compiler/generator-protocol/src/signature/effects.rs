use super::*;

impl SignatureProjection<'_> {
    pub(crate) fn effects(
        &self,
        is_suspend: bool,
        modifiers: &hir::CallableModifiers,
        attributes: &hir::FunctionAttributes,
        release_callability: &hir::ReleaseCallability,
        span: hir::Span,
    ) -> Result<Value> {
        let release = match release_callability {
            hir::ConditionalReleaseCallability::Unavailable => json!({"kind": "unavailable"}),
            hir::ConditionalReleaseCallability::NoTransition { requirements } => {
                let requirements = requirements
                    .iter()
                    .map(|parameter| {
                        let binder = self
                            .binders
                            .iter()
                            .find(|binder| binder.parameter == *parameter)
                            .ok_or_else(|| {
                                error(span, "release condition has no declaration binder")
                            })?;
                        Ok(json!({"kind": "binder", "depth": binder.depth, "index": binder.index}))
                    })
                    .collect::<Result<Vec<_>>>()?;
                json!({"kind": "noTransition", "requirements": requirements})
            }
        };
        Ok(json!({
            "execution": if is_suspend { "suspend" } else { "ordinary" },
            "safety": QueryJson(&attributes.safety),
            "gc": QueryJson(&attributes.gc_effect),
            "callingConvention": match attributes.calling_convention { hir::CallingConvention::Cdecl => "c" },
            "operator": QueryJson(&modifiers.operator),
            "propertyDelegateOperator": QueryJson(&modifiers.property_delegate_operator),
            "infix": modifiers.is_infix,
            "release": release,
        }))
    }
}
