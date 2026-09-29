use super::*;

#[derive(Clone, Copy)]
pub(in crate::expr) enum ImportedGenericTarget {
    Function(hir::ImportedGenericCallableTemplateId),
    Constructor(hir::ImportedConstructorTemplateId),
}

pub(in crate::expr) struct ImportedInferenceSignature {
    pub owner_parameters: Vec<hir::TypeParamDecl>,
    pub type_parameters: Vec<hir::TypeParamDecl>,
    pub parameters: Vec<(String, hir::TypeId)>,
    pub receiver: Option<hir::TypeId>,
    pub return_type: hir::TypeId,
    pub origin: hir::DefinitionOrigin,
    pub span: hir::Span,
}

impl ImportedGenericTarget {
    pub(in crate::expr::named_calls::imported_dependency) fn request(
        state: &mut Lowerer,
        declaration: hir::ImportedCallableDeclaration,
    ) -> Result<Self, String> {
        if matches!(
            declaration.interface().declaration(),
            scoop_identity::CallableTemplateOrigin::Constructor(_)
        ) {
            state
                .request_imported_constructor_template(declaration)
                .map(Self::Constructor)
        } else {
            state
                .request_imported_generic_template(declaration)
                .map(Self::Function)
        }
    }

    pub(in crate::expr::named_calls::imported_dependency) fn declaration(
        self,
        state: &Lowerer,
    ) -> hir::ImportedCallableDeclaration {
        match self {
            Self::Function(id) => state.imported_generic_templates[id]
                .source
                .declaration()
                .clone(),
            Self::Constructor(id) => state.imported_constructor_templates[id].source.clone(),
        }
    }

    pub(in crate::expr) fn signature(
        self,
        state: &Lowerer,
    ) -> (
        ImportedInferenceSignature,
        crate::imported_core::ImportedTypeBindings,
    ) {
        match self {
            Self::Function(id) => {
                let template = &state.imported_generic_templates[id];
                let owner_count = match template.declaration {
                    hir::ImportedCallableTemplateOrigin::Nominal {
                        owner_parameter_count,
                        ..
                    } => owner_parameter_count,
                    _ => 0,
                };
                let (owner, callable) = template
                    .type_parameters
                    .declarations()
                    .split_at(owner_count);
                (
                    ImportedInferenceSignature {
                        owner_parameters: owner.to_vec(),
                        type_parameters: callable.to_vec(),
                        parameters: template
                            .parameters
                            .iter()
                            .map(|p| (p.name.clone(), p.ty))
                            .collect(),
                        receiver: template.receiver,
                        return_type: template.return_type,
                        origin: template.origin,
                        span: template.span,
                    },
                    template.bindings.clone(),
                )
            }
            Self::Constructor(id) => {
                let template = &state.imported_constructor_templates[id];
                let signature = &template.signature;
                (
                    ImportedInferenceSignature {
                        owner_parameters: Vec::new(),
                        type_parameters: signature.type_parameters.clone(),
                        parameters: signature
                            .parameters
                            .iter()
                            .map(|p| (p.name.clone(), p.ty))
                            .collect(),
                        receiver: None,
                        return_type: signature.owner,
                        origin: signature.origin,
                        span: signature.origin.span,
                    },
                    template.bindings.clone(),
                )
            }
        }
    }
}
