use super::*;
use hir::{EditingCallee as C, SemanticConstructor as Constructor};

impl DeclarationAnalysis {
    pub fn body_parameter(
        &self,
        body: &hir::EditingSourceBody,
        definition: &hir::SemanticSourceLocation,
    ) -> Option<(hir::EditingCallee, usize)> {
        let index = self
            .body_parameters(body)
            .iter()
            .position(|origin| origin == definition);
        body.callee()
            .zip(index)
            .or_else(|| self.lowerer.editing_parameter_at_origin(definition))
    }

    pub(in crate::editing) fn body_parameters(
        &self,
        body: &hir::EditingSourceBody,
    ) -> Vec<hir::SemanticSourceLocation> {
        if let Some(signature) = self.source_body_signature(body) {
            return match signature {
                V::Known(signature) => signature.parameters.into_iter().map(|p| p.origin).collect(),
                V::Unavailable(_) => Vec::new(),
            };
        }
        let Some(callee) = body.callee() else {
            return Vec::new();
        };
        let count = match callee {
            C::Constructor(Constructor::CurrentClass(id)) => {
                self.lowerer.class_constructors[id].parameters.len()
            }
            C::Constructor(Constructor::CurrentStruct(id)) => {
                self.lowerer.struct_constructors[id].parameters.len()
            }
            C::CurrentVariant(variant) => self.lowerer.enums[variant.enumeration()].variants
                [variant.local_index() as usize]
                .fields
                .len(),
            _ => return Vec::new(),
        };
        (0..count)
            .map(|index| {
                self.lowerer
                    .editing_callee_parameter_origin(callee, index)
                    .expect("a source constructor retains each parameter definition")
            })
            .collect()
    }
}
