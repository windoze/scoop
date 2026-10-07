//! Finalize fixture declarations with explicit local and imported providers.

use super::*;

impl Builder {
    pub(in crate::tests) fn finish(self, entry: mir::FunctionId) -> mir::Module {
        self.finish_with_types(entry, ConeIdentity::SINGLE_FILE, Vec::new())
    }

    pub(in crate::tests) fn finish_with_types(
        mut self,
        entry: mir::FunctionId,
        provider: ConeIdentity,
        mut exact_types: Vec<mir::SourceExactTypeIdentity>,
    ) -> mir::Module {
        for (name, representation) in mir::IntegerKind::ALL
            .map(|kind| {
                (
                    kind.canonical_name(),
                    mir::IntrinsicTypeRepresentation::Integer(kind),
                )
            })
            .into_iter()
            .chain([("Boolean", mir::IntrinsicTypeRepresentation::Boolean)])
        {
            self.structs.alloc(mir::StructDef {
                name: name.to_string(),
                type_arguments: Vec::new(),
                gc_free: true,
                representation: mir::StructRepresentation::Intrinsic(representation),
            });
        }
        self.classes.alloc(mir::ClassDef {
            release_policy: Default::default(),
            modifier: mir::ClassModifier::Final,
            name: "String".to_string(),
            type_arguments: Vec::new(),
            representation: mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::String,
            ),
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        test_exact_type_at(
            provider,
            &self.function_types,
            &mir::Type::String,
            &mut exact_types,
        );
        for (id, structure) in self.structs.iter() {
            let ty = match &structure.representation {
                mir::StructRepresentation::Declared { .. } => mir::Type::Struct(id),
                mir::StructRepresentation::Intrinsic(representation) => match representation {
                    mir::IntrinsicTypeRepresentation::Integer(kind) => mir::Type::Integer(*kind),
                    mir::IntrinsicTypeRepresentation::Float(_)
                    | mir::IntrinsicTypeRepresentation::Char => mir::Type::Struct(id),
                    mir::IntrinsicTypeRepresentation::Unit => mir::Type::Unit,
                    mir::IntrinsicTypeRepresentation::Boolean => mir::Type::Boolean,
                    mir::IntrinsicTypeRepresentation::Ptr { pointee } => {
                        mir::Type::Ptr(Box::new(pointee.clone()))
                    }
                    mir::IntrinsicTypeRepresentation::FunPtr { signature } => {
                        mir::Type::FunPtr(*signature)
                    }
                    mir::IntrinsicTypeRepresentation::String
                    | mir::IntrinsicTypeRepresentation::Any
                    | mir::IntrinsicTypeRepresentation::Nothing
                    | mir::IntrinsicTypeRepresentation::Array { .. }
                    | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                        unreachable!("the test registry fixes intrinsic declaration targets")
                    }
                },
            };
            test_exact_type_at(provider, &self.function_types, &ty, &mut exact_types);
            if let mir::StructRepresentation::Declared { fields, .. } = &structure.representation {
                for field in fields {
                    test_tuple_layout_type(
                        provider,
                        &self.function_types,
                        &field.ty,
                        &mut exact_types,
                    );
                }
            }
        }
        for (id, enumeration) in self.enums.iter() {
            test_exact_type_at(
                provider,
                &self.function_types,
                &mir::Type::Enum(id, enumeration.type_arguments.clone()),
                &mut exact_types,
            );
            for variant in &enumeration.variants {
                for field in &variant.fields {
                    test_tuple_layout_type(
                        provider,
                        &self.function_types,
                        &field.ty,
                        &mut exact_types,
                    );
                }
            }
        }
        for (id, _) in self.interfaces.iter() {
            test_exact_type_at(
                provider,
                &self.function_types,
                &mir::Type::Interface(id),
                &mut exact_types,
            );
        }
        for (id, class) in self.classes.iter() {
            if !matches!(
                class.representation,
                mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
            ) {
                test_exact_type_at(
                    provider,
                    &self.function_types,
                    &mir::Type::Class(id),
                    &mut exact_types,
                );
            }
            if let mir::ClassRepresentation::Declared { fields, .. } = &class.representation {
                for field in fields {
                    test_tuple_layout_type(
                        provider,
                        &self.function_types,
                        &field.ty,
                        &mut exact_types,
                    );
                }
            }
        }
        let mut source_callables = Vec::new();
        for (declaration_index, &function_id) in self.top_level.iter().enumerate() {
            let function = &self.functions[function_id];
            for (_, local) in function.body.locals.iter() {
                test_tuple_layout_type(provider, &self.function_types, &local.ty, &mut exact_types);
            }
            let parameters = function
                .params
                .iter()
                .map(|parameter| {
                    test_exact_type_at(
                        provider,
                        &self.function_types,
                        &parameter.ty,
                        &mut exact_types,
                    )
                })
                .collect();
            let result = test_exact_type_at(
                provider,
                &self.function_types,
                &function.return_ty,
                &mut exact_types,
            );
            let declaration_name = if function_id == entry {
                "main".to_string()
            } else {
                format!("testFunction{declaration_index}")
            };
            let declaration = SourceDeclarationKey::function(
                test_declaration_site(provider),
                CanonicalIdentifier::new(&declaration_name).unwrap(),
                0,
                None,
                Vec::new(),
            );
            let owner = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
            let materialization = CallableMaterialization::new(
                CallableTemplateOwner::Function(owner),
                CallableMaterializationContext::NoSubstitution,
            );
            source_callables.push(
                mir::SourceCallableMaterialization::new(
                    function_id,
                    materialization,
                    ExactCallableSignature::new(Effect::Ordinary, None, parameters, result),
                    None,
                )
                .unwrap(),
            );
        }
        let source_exact_types = mir::SourceExactTypeIdentities::checked(exact_types).unwrap();
        let source_callable_materializations =
            mir::SourceCallableMaterializations::checked(source_callables).unwrap();
        let task = mir::ContextStorageType::new(provider, mir::ContextStorageRole::Task);
        let generated_exact_types = mir::GeneratedExactTypeIdentities::checked(vec![
            mir::GeneratedExactTypeIdentity::new(
                mir::GeneratedExactTypeLocation::Context(task),
                &task.nominal_record(),
                None,
            )
            .unwrap(),
        ])
        .unwrap();
        let callable_signatures = mir::MirCallableSignatures::checked(
            source_callable_materializations
                .iter()
                .map(|source| source.signature_record().clone())
                .collect(),
        )
        .unwrap();
        mir::Module {
            release_hooks: Arena::new(),
            cone: provider,
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: Arena::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            callback_bridges: Arena::new(),
            foreign_callback_adapters: Arena::new(),
            foreign_callback_families: Arena::new(),
            foreign_callback_bridges: Arena::new(),
            option_core: self.option_core,
            function_types: self.function_types,
            closure_classes: Arena::new(),
            closure_invoke_functions: Arena::new(),
            top_level: self.top_level,
            strings: self.strings,
            structs: self.structs,
            enums: self.enums,
            classes: self.classes,
            interfaces: self.interfaces,
            output: mir::MirOutput::Executable { entry },
            meta: mir::MirMeta {
                generated_exact_types,
                source_exact_types,
                source_callable_materializations,
                callable_signatures,
                ..mir::MirMeta::default()
            },
        }
    }
}
