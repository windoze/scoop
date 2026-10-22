use super::*;

impl SignatureProjection<'_> {
    pub(super) fn effects(&self, signature: &hir::SemanticFunctionSignature) -> Result<Value> {
        let release = match &signature.release_callability {
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
                                error(
                                    signature.origin.span,
                                    "release condition has no declaration binder",
                                )
                            })?;
                        Ok(json!({"kind": "binder", "depth": binder.depth, "index": binder.index}))
                    })
                    .collect::<Result<Vec<_>>>()?;
                json!({"kind": "noTransition", "requirements": requirements})
            }
        };
        Ok(json!({
            "execution": if signature.is_suspend { "suspend" } else { "ordinary" },
            "safety": QueryJson(&signature.attributes.safety),
            "gc": QueryJson(&signature.attributes.gc_effect),
            "callingConvention": match signature.attributes.calling_convention { hir::CallingConvention::Cdecl => "c" },
            "operator": QueryJson(&signature.modifiers.operator),
            "propertyDelegateOperator": QueryJson(&signature.modifiers.property_delegate_operator),
            "infix": signature.modifiers.is_infix,
            "release": release,
        }))
    }
}
