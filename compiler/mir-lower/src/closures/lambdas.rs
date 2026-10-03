use super::*;

impl Lowerer {
    pub(super) fn declare_lambdas(
        &mut self,
        module: &hir::Module,
        definitions: &mut ClosureDefinitions,
    ) {
        for (id, lambda) in module.lambdas.iter() {
            let function_type = self.lower_function_type_id(module, lambda.function_type);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let semantic_fields = lambda
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
            let identity = mir::ClosureEnvironmentIdentity::for_lambda(
                module.functions[lambda.function].materialization,
                lambda
                    .captures
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        (
                            capture_source(index),
                            module
                                .local_value_identities
                                .lambda_capture(id, index)
                                .clone(),
                        )
                    })
                    .collect(),
                materialization_odr_group(
                    module,
                    module.functions[lambda.function].materialization,
                ),
            )
            .expect("LocalConcrete lambda captures have complete persistent identities");
            let captures = order_closure_fields(&identity, semantic_fields);
            let definition = ClosureDefinition::Body(lambda.function);
            if let Some(class) = definitions.lookup(
                &self.closure_classes,
                &identity,
                function_type,
                &captures,
                &definition,
            ) {
                self.index_closure_captures(class, &identity, &lambda.captures);
                self.lambda_closures.insert(id, class);
                continue;
            }
            let invoke_function = self.function_map[&lambda.function];
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: invoke_function,
            });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$lambda{}", id.into_raw()),
                function_type,
                invoke,
                captures,
                bridges: Vec::new(),
            });
            self.closure_by_function.insert(lambda.function, class);
            self.index_closure_captures(class, &identity, &lambda.captures);
            definitions.insert(class, identity.clone(), definition);
            self.closure_environments.push(
                mir::ClosureEnvironment::checked(class, &self.closure_classes[class], identity)
                    .expect("lambda closure identity covers every physical field"),
            );
            self.lambda_closures.insert(id, class);
        }
    }
}
