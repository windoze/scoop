use super::*;

impl FrontendQueries<'_, '_> {
    pub(in crate::semantic_queries) fn query_default_reference(
        &mut self,
        owner: hir::DecodedAnnotationCallableV1,
        parameter: u32,
    ) -> Result<Option<hir::SemanticDefaultReference>, Diagnostic> {
        let Some(declaration) =
            self.query_declaration(hir::DecodedAnnotationTargetV1::Parameter {
                callable: owner,
                index: parameter,
            })?
        else {
            return Ok(None);
        };
        let source = match declaration.annotations {
            hir::SemanticAnnotationTarget::Current(hir::SourceAnnotationTarget::Parameter {
                callable,
                index,
            }) => {
                let owner = match callable {
                    hir::SourceAnnotationCallable::Function(id) => {
                        hir::ExportParameterOwner::Function(id)
                    }
                    hir::SourceAnnotationCallable::StructConstructor(id) => {
                        hir::ExportParameterOwner::StructConstructor(id)
                    }
                    hir::SourceAnnotationCallable::ClassConstructor(id) => {
                        hir::ExportParameterOwner::ClassConstructor(id)
                    }
                    hir::SourceAnnotationCallable::VariantConstructor(id) => {
                        hir::ExportParameterOwner::VariantConstructor(id)
                    }
                    hir::SourceAnnotationCallable::Getter(_)
                    | hir::SourceAnnotationCallable::Setter(_) => return Ok(None),
                };
                self.lowerer
                    .source_parameter_interfaces
                    .iter()
                    .find(|interface| interface.owner == owner)
                    .and_then(|interface| interface.parameters.get(index as usize))
                    .and_then(|parameter| match parameter.calling {
                        hir::ExportParameterCalling::Default { source, .. }
                        | hir::ExportParameterCalling::Vararg {
                            omission: hir::ExportVarargOmission::Default(source),
                            ..
                        } => Some(hir::SemanticDefaultReference::Current(source)),
                        _ => None,
                    })
            }
            hir::SemanticAnnotationTarget::Imported(hir::AnnotationTargetV1::Parameter {
                callable: hir::AnnotationCallableV1::Declaration(owner),
                index,
            }) => {
                let key = hir::ExportDefaultTemplateKeyV1::new(owner, index);
                self.lowerer
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.default_template(key))
                    .map(|_| hir::SemanticDefaultReference::Imported(key))
            }
            hir::SemanticAnnotationTarget::Imported(hir::AnnotationTargetV1::Parameter {
                callable: hir::AnnotationCallableV1::Local(_),
                ..
            }) => None,
            _ => unreachable!("a resolved parameter retains its parameter target"),
        };
        Ok(source)
    }
}
