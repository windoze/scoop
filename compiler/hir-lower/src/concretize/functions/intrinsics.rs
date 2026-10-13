use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_intrinsic_parameters(
        &mut self,
        key: &FunctionKey,
        arguments: &[concrete::TypeId],
        receiver: &DefinitionReceiver,
    ) -> Vec<concrete::TypeId> {
        let mut parameters = match receiver {
            DefinitionReceiver::None => Vec::new(),
            DefinitionReceiver::Extension(ty) | DefinitionReceiver::Method { owner: ty, .. } => {
                vec![*ty]
            }
        };
        match self.function_source(key) {
            FunctionSource::Local(function) => {
                let interface = self
                    .source
                    .source_parameter_interfaces
                    .iter()
                    .find(|interface| {
                        interface.owner == export::ExportParameterOwner::Function(function)
                    })
                    .expect("a source intrinsic retains its resolved parameter interface");
                parameters.extend(
                    interface
                        .parameters
                        .iter()
                        .map(|param| match param.calling {
                            export::ExportParameterCalling::Required { value_type }
                            | export::ExportParameterCalling::Default { value_type, .. } => {
                                value_type
                            }
                            export::ExportParameterCalling::Vararg { parameter_type, .. } => {
                                self.source.export_vararg_parameter_types[parameter_type].array_type
                            }
                        }),
                );
            }
            FunctionSource::Imported(template) => {
                parameters.extend(
                    self.source.imported_generic_templates[template]
                        .params
                        .iter()
                        .map(|param| param.ty),
                );
            }
            FunctionSource::Companion(..) => {
                unreachable!("an initialization companion is not an intrinsic")
            }
        }
        parameters
            .into_iter()
            .map(|ty| self.lower_type(ty, arguments))
            .collect()
    }
}
