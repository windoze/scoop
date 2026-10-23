use super::*;

impl MemberProjection<'_> {
    pub(super) fn parameters(
        &self,
        parameters: &[hir::SemanticParameterDescription],
    ) -> Result<Vec<Value>> {
        parameters
            .iter()
            .map(|parameter| self.parameter(parameter))
            .collect()
    }

    fn parameter(&self, parameter: &hir::SemanticParameterDescription) -> Result<Value> {
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
                json!({"kind": "vararg", "elementType": self.type_use(*element_type)?, "omission": omission})
            }
        };
        Ok(json!({
            "declaration": {"owner": CallableDeclaration::from(parameter.identity.owner), "index": parameter.identity.index},
            "name": parameter.name, "valueType": self.type_use(parameter.value_type)?, "calling": calling,
            "target": QueryJson(&parameter.target), "annotations": self.annotations(parameter.annotations),
            "origin": QueryJson(&parameter.origin),
        }))
    }

    fn default_use(
        &self,
        parameter: hir::SemanticParameterIdentity,
        default: &hir::SemanticDefaultUse,
    ) -> Result<Value> {
        let mut reference = self.applied.default_use(parameter, default)?;
        let arguments = default
            .declaration_arguments
            .iter()
            .map(|&ty| self.declaration.ty(ty))
            .collect::<Result<Vec<_>>>()?;
        reference["declarationArguments"] = json!(arguments);
        Ok(reference)
    }
}
