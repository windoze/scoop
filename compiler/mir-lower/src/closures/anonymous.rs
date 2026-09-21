use super::*;

impl Lowerer {
    pub(super) fn declare_anonymous(
        &mut self,
        module: &hir::Module,
        definitions: &mut ClosureDefinitions,
    ) {
        for (id, anonymous) in module.anonymous_functions.iter() {
            let function_type = self.lower_function_type_id(module, anonymous.function_type);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let semantic_fields = anonymous
                .captures
                .iter()
                .enumerate()
                .map(|(index, capture)| {
                    (
                        capture_source(index),
                        mir::Field {
                            name: capture.name.clone(),
                            ty: types.lower(
                                capture.ty,
                                &mut self.source_exact_types,
                                &mut self.enums,
                                &mut self.structs,
                                &mut self.interfaces,
                                &mut self.shell,
                            ),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let identity = mir::ClosureEnvironmentIdentity::for_anonymous_function(
                module.functions[anonymous.function].materialization,
                anonymous
                    .captures
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        (
                            capture_source(index),
                            module
                                .local_value_identities
                                .anonymous_function_capture(id, index)
                                .clone(),
                        )
                    })
                    .collect(),
                materialization_odr_group(
                    module,
                    module.functions[anonymous.function].materialization,
                ),
            )
            .expect("LocalConcrete anonymous captures have complete persistent identities");
            let captures = order_closure_fields(&identity, semantic_fields);
            let definition = ClosureDefinition::Body(anonymous.function);
            if let Some(class) = definitions.lookup(
                &self.closure_classes,
                &identity,
                function_type,
                &captures,
                &definition,
            ) {
                self.index_closure_captures(class, &identity, &anonymous.captures);
                self.anonymous_closures.insert(id, class);
                continue;
            }
            let invoke_function = self.function_map[&anonymous.function];
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: invoke_function,
            });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$anonymous{}", id.into_raw()),
                function_type,
                invoke,
                captures,
                bridges: Vec::new(),
            });
            self.closure_by_function.insert(anonymous.function, class);
            self.index_closure_captures(class, &identity, &anonymous.captures);
            definitions.insert(class, identity.clone(), definition);
            self.closure_environments.push(
                mir::ClosureEnvironment::checked(class, &self.closure_classes[class], identity)
                    .expect("anonymous closure identity covers every physical field"),
            );
            self.anonymous_closures.insert(id, class);
        }
    }
}
