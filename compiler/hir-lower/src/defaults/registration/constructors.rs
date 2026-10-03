use super::*;

impl Lowerer {
    pub(super) fn register_constructor_parameter_interface(
        &mut self,
        owner: hir::ExportParameterOwner,
        source_parameters: &[ast::Param],
        parameters: Vec<hir::ConstructorParameter>,
        type_parameters: Vec<hir::TypeParamDecl>,
        callable_name: String,
    ) {
        let source_owner = match owner {
            hir::ExportParameterOwner::StructConstructor(constructor) => {
                SourceParameterOwner::StructConstructor(constructor)
            }
            hir::ExportParameterOwner::ClassConstructor(constructor) => {
                SourceParameterOwner::ClassConstructor(constructor)
            }
            _ => unreachable!("constructor helper receives a constructor owner"),
        };
        let callings = match source_owner {
            SourceParameterOwner::StructConstructor(constructor) => {
                self.struct_parameter_calling.get(&constructor)
            }
            SourceParameterOwner::ClassConstructor(constructor) => {
                self.class_parameter_calling.get(&constructor)
            }
            _ => unreachable!("constructor helper receives a constructor owner"),
        }
        .cloned()
        .unwrap_or_default();
        if source_parameters.len() != parameters.len() || parameters.len() != callings.len() {
            return;
        }
        let sources = source_parameters
            .iter()
            .zip(parameters.iter().zip(callings))
            .map(|(source, (parameter, calling))| ParameterSource {
                name: source.name.clone(),
                ty: parameter.ty,
                calling,
            })
            .collect::<Vec<_>>();
        self.register_export_parameter_interface(
            owner,
            &sources,
            &DefaultContext {
                definition_root: match owner {
                    hir::ExportParameterOwner::StructConstructor(constructor) => {
                        hir::LexicalDefinitionRoot::StructConstructor(constructor)
                    }
                    hir::ExportParameterOwner::ClassConstructor(constructor) => {
                        hir::LexicalDefinitionRoot::ClassConstructor(constructor)
                    }
                    hir::ExportParameterOwner::Function(_)
                    | hir::ExportParameterOwner::VariantConstructor(_) => {
                        unreachable!("constructor helper receives a constructor owner")
                    }
                },
                source_context: match owner {
                    hir::ExportParameterOwner::StructConstructor(constructor) => {
                        hir::SourceContextSubject::Constructor(
                            hir::SourceContextConstructor::Struct(constructor),
                        )
                    }
                    hir::ExportParameterOwner::ClassConstructor(constructor) => {
                        hir::SourceContextSubject::Constructor(
                            hir::SourceContextConstructor::Class(constructor),
                        )
                    }
                    hir::ExportParameterOwner::Function(_)
                    | hir::ExportParameterOwner::VariantConstructor(_) => {
                        unreachable!("constructor helper receives a constructor owner")
                    }
                },
                type_parameters,
                receiver: None,
                is_suspend: false,
                safety: match owner {
                    hir::ExportParameterOwner::ClassConstructor(id) => {
                        self.class_constructors[id].safety
                    }
                    hir::ExportParameterOwner::StructConstructor(id) => {
                        self.struct_constructors[id].safety
                    }
                    _ => unreachable!("constructor helper receives a constructor owner"),
                },
                callable_name,
            },
        );
    }
}
