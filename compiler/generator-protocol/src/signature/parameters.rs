use super::*;

impl SignatureProjection<'_> {
    pub(crate) fn parameter(&self, parameter: &hir::SemanticParameterDescription) -> Result<Value> {
        let calling = match &parameter.calling {
            hir::SemanticParameterCalling::Required => json!({"kind": "required"}),
            hir::SemanticParameterCalling::Default(default) => json!({
                "kind": "default", "default": self.default_use(parameter.identity, default)?,
            }),
            hir::SemanticParameterCalling::Vararg {
                element_type,
                omission,
            } => {
                let omission = match omission {
                    hir::SemanticVarargOmission::EmptyArray => json!({"kind": "emptyArray"}),
                    hir::SemanticVarargOmission::Default(default) => json!({
                        "kind": "default", "default": self.default_use(parameter.identity, default)?,
                    }),
                };
                json!({"kind": "vararg", "elementType": self.ty(element_type.declaration)?, "omission": omission})
            }
        };
        Ok(json!({
            "declaration": {"owner": CallableDeclaration::from(parameter.identity.owner), "index": parameter.identity.index},
            "name": parameter.name, "valueType": self.ty(parameter.value_type.declaration)?,
            "calling": calling, "origin": QueryJson(&parameter.origin),
        }))
    }

    pub(crate) fn default_use(
        &self,
        parameter: hir::SemanticParameterIdentity,
        default: &hir::SemanticDefaultUse,
    ) -> Result<Value> {
        let arguments = default
            .declaration_arguments
            .iter()
            .map(|&ty| self.ty(ty))
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({
            "owner": CallableDeclaration::from(parameter.owner), "parameter": parameter.index,
            "arguments": arguments,
        }))
    }
}
