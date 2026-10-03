use super::*;
use crate::FnVarargOmission;

impl Lowerer {
    pub(in crate::defaults) fn finish_export_parameter_interfaces(&mut self) {
        let owners = std::mem::take(&mut self.default_preparation.export_order);
        let outer_file = self.current_file;
        for &owner in &owners {
            let owner = lowering::source_owner(owner);
            self.current_file = self.default_preparation.recipes[&owner].file;
            self.prepare_parameter_defaults(owner);
        }
        for owner in owners {
            let source_owner = lowering::source_owner(owner);
            let recipe = self.default_preparation.recipes[&source_owner].clone();
            self.current_file = recipe.file;
            let mut parameters = Vec::with_capacity(recipe.sources.len());
            let mut complete = true;
            for (index, parameter) in recipe.sources.iter().enumerate() {
                let key = SourceDefaultKey::new(source_owner, index);
                let template =
                    self.default_templates
                        .get(&key.tuple())
                        .map(|source| match source {
                            DefaultExprTemplateRef::Export(source) => *source,
                            DefaultExprTemplateRef::Local(_) => {
                                unreachable!("exported parameters use exported defaults")
                            }
                        });
                if matches!(
                    self.default_preparation.states.get(&key),
                    Some(PreparationState::Failed)
                ) {
                    complete = false;
                    continue;
                }
                let calling = match &parameter.calling {
                    FnParamCalling::Required => match template {
                        Some(source) => hir::ExportParameterCalling::Default {
                            value_type: parameter.ty,
                            source,
                        },
                        None => hir::ExportParameterCalling::Required {
                            value_type: parameter.ty,
                        },
                    },
                    FnParamCalling::Default { .. } => hir::ExportParameterCalling::Default {
                        value_type: parameter.ty,
                        source: template.expect("a successful declared default is prepared"),
                    },
                    FnParamCalling::Vararg {
                        element_ty,
                        omission,
                    } => {
                        let parameter_type = self.export_vararg_parameter_types.alloc(
                            hir::ExportVarargParameterType {
                                element_type: *element_ty,
                                array_type: parameter.ty,
                            },
                        );
                        let omission = match (template, omission) {
                            (Some(source), _) => hir::ExportVarargOmission::Default(source),
                            (None, FnVarargOmission::EmptyArray) => {
                                hir::ExportVarargOmission::EmptyArray
                            }
                            (None, FnVarargOmission::Default { .. }) => {
                                unreachable!("a successful declared default is prepared")
                            }
                        };
                        hir::ExportParameterCalling::Vararg {
                            parameter_type,
                            omission,
                        }
                    }
                };
                parameters.push(hir::ExportValueParameter {
                    name: parameter.name.text.clone(),
                    calling,
                    origin: self.definition_origin(parameter.name.span),
                });
            }
            if complete {
                self.source_parameter_interfaces
                    .push(hir::ExportParameterInterface { owner, parameters });
            }
        }
        self.current_file = outer_file;
    }
}
