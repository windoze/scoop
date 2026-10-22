use super::*;

impl SignatureProjection<'_> {
    pub(super) fn type_parameter(
        &self,
        parameter: &hir::TypeParamDecl,
        binder: &hir::HirSignatureBinder,
        reference: Value,
    ) -> Result<Value> {
        let companion = parameter
            .companion
            .as_ref()
            .map(|companion| -> Result<Value> {
                Ok(json!({
                    "bounds": self.bounds(&companion.bounds)?,
                    "excludedInterfaces": self.excluded(&companion.excluded_interfaces)?,
                    "origin": self.origin(companion.span),
                }))
            })
            .transpose()?;
        let requirements = parameter
            .requirements
            .iter()
            .map(|requirement| {
                let contract = requirement
                    .contract
                    .key(|ty| self.queries.type_signature(self.context, ty))?;
                Ok(json!({
                    "index": requirement.id.ordinal, "contract": QueryJson(&contract),
                    "origin": self.origin(requirement.span),
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({
            "depth": binder.depth, "index": binder.index,
            "name": parameter.name, "declaration": reference,
            "bounds": self.bounds(&parameter.bounds)?,
            "excludedInterfaces": self.excluded(&parameter.excluded_interfaces)?,
            "companion": companion, "requirements": requirements,
            "origin": self.origin(parameter.span),
        }))
    }

    fn bounds(&self, bounds: &hir::TypeParamBounds) -> Result<Value> {
        Ok(match bounds {
            hir::TypeParamBounds::Unconstrained => json!({"kind": "unconstrained"}),
            hir::TypeParamBounds::Value { span } => {
                json!({"kind": "value", "origin": self.origin(*span)})
            }
            hir::TypeParamBounds::Ref { span } => {
                json!({"kind": "ref", "origin": self.origin(*span)})
            }
            hir::TypeParamBounds::Nominal(bounds) => {
                let bounds = bounds.in_source_order().iter().map(|bound| {
                    let kind = match bound {
                        hir::NominalBoundRef::Class(_) => "class",
                        hir::NominalBoundRef::Interface(_) => "interface",
                    };
                    Ok(json!({"kind": kind, "valueType": self.ty(bound.ty())?, "origin": self.origin(bound.span())}))
                }).collect::<Result<Vec<_>>>()?;
                json!({"kind": "nominal", "bounds": bounds})
            }
        })
    }

    fn excluded(&self, interfaces: &[hir::InterfaceUpperBound]) -> Result<Vec<Value>> {
        interfaces
            .iter()
            .map(|bound| {
                Ok(json!({"valueType": self.ty(bound.ty)?, "origin": self.origin(bound.span)}))
            })
            .collect()
    }
}
