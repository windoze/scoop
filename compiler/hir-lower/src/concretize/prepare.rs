use super::*;

pub(super) struct PreparedSource {
    pub unit: concrete::TypeId,
    pub boolean: concrete::TypeId,
    pub string: concrete::TypeId,
    pub native_callback_signatures: Vec<concrete::NativeCallbackSignature>,
    pub core_protocols: concrete::ConcreteCoreProtocols,
}

impl Concretizer<'_> {
    pub(super) fn prepare_source(&mut self) -> PreparedSource {
        let unit = self.lower_type(self.source.unit, &[]);
        match self.core {
            export::CoreProtocols::Defined(protocols) => {
                for kind in export::IntegerKind::ALL {
                    let owner = protocols.fundamental_types.integers.owner(kind);
                    let source_type = self.source.struct_applications
                        [self.source.structs[owner].self_application]
                        .canonical_type;
                    self.lower_type(source_type, &[]);
                }
            }
            export::CoreProtocols::Imported(_) => {
                let integer_types = self
                    .source
                    .types
                    .iter()
                    .filter_map(|(id, ty)| matches!(ty, export::Type::Integer(_)).then_some(id))
                    .collect::<Vec<_>>();
                for ty in integer_types {
                    self.lower_type(ty, &[]);
                }
            }
        }
        let boolean = self.lower_type(self.source.boolean, &[]);
        let string = self.lower_type(self.source.string, &[]);

        self.lower_extern_functions();
        self.lower_globals();
        for (id, declaration) in self.source.structs.iter() {
            if declaration.type_params.is_empty()
                && self.automatic_nominal(&self.source.nominal_identities[id])
            {
                self.lower_struct_application(declaration.self_application, &[]);
                self.lower_public_equality(
                    self.source.struct_applications[declaration.self_application].canonical_type,
                );
            }
        }
        for (id, declaration) in self.source.enums.iter() {
            if declaration.type_params.is_empty()
                && self.automatic_nominal(&self.source.nominal_identities[id])
            {
                self.ensure_enum(id, Vec::new());
                self.lower_public_equality(
                    self.source.enum_applications[declaration.self_application].canonical_type,
                );
            }
        }
        for (id, declaration) in self.source.interfaces.iter() {
            if declaration.type_params.is_empty()
                && self.automatic_nominal(&self.source.nominal_identities[id])
            {
                self.ensure_interface(id, Vec::new());
            }
        }
        for (id, declaration) in self.source.classes.iter() {
            if declaration.type_params.is_empty() && self.automatic_class(id) {
                self.lower_class_application(declaration.self_application, &[]);
            }
        }
        let lexical_functions =
            self.source
                .local_functions
                .iter()
                .filter_map(|(_, local)| local.source().map(|(function, _)| function))
                .chain(self.source.lambdas.iter().filter_map(|(_, lambda)| {
                    lambda.definition.source().map(|(function, _)| function)
                }))
                .chain(
                    self.source
                        .anonymous_functions
                        .iter()
                        .filter_map(|(_, anonymous)| {
                            anonymous.definition.source().map(|(function, _)| function)
                        }),
                )
                .collect::<std::collections::HashSet<_>>();
        for (id, function) in self.source.functions.iter() {
            if !lexical_functions.contains(&id)
                && function.method.is_none()
                && function.type_param_count() == 0
                && self.is_emittable_source_function(id)
                && self.initialization_helper_is_required(id)
            {
                self.request_function(id, Vec::new());
            }
        }
        self.drain_pending_callables();

        let native_callback_signatures = self
            .source
            .native_callback_signatures
            .iter()
            .map(|callback| {
                let function = self.request_function(callback.function, Vec::new());
                let signature = self.lower_function_type(callback.signature, &[]);
                self.prepare_callback_storage_types(signature);
                self.intern_type(concrete::TypeKind::FunPtr(signature), true);
                concrete::NativeCallbackSignature {
                    function,
                    signature,
                }
            })
            .collect();
        self.drain_pending_callables();

        let core_protocols = match self.core {
            export::CoreProtocols::Defined(protocols) => {
                self.lower_defined_core_protocols(protocols)
            }
            export::CoreProtocols::Imported(protocols) => {
                concrete::ConcreteCoreProtocols::Imported(protocols.clone())
            }
        };
        PreparedSource {
            unit,
            boolean,
            string,
            native_callback_signatures,
            core_protocols,
        }
    }
}
