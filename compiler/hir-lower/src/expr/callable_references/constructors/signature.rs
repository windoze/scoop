use super::*;

impl ConstructorReferenceDeclaration {
    pub(super) fn resolve(
        state: &mut Lowerer,
        candidate: ConstructorReferenceCandidate,
        span: Span,
    ) -> Result<Self, String> {
        let (target, mut signature, safety, outer_type) = match &candidate.source {
            ConstructorReferenceSource::Local(source) => {
                let safety = match source {
                    NominalConstructorSource::Class(id) => state.class_constructors[*id].safety,
                    NominalConstructorSource::Struct(id) => state.struct_constructors[*id].safety,
                    _ => hir::Safety::Safe,
                };
                (
                    ConstructorReferenceTarget::Local(*source),
                    state.nominal_constructor_view(*source, span).signature,
                    safety,
                    match source {
                        NominalConstructorSource::Class(id) => state.class_constructors[*id]
                            .enclosing_parameter
                            .as_ref()
                            .map(|parameter| parameter.ty),
                        _ => None,
                    },
                )
            }
            ConstructorReferenceSource::Imported(declaration) => {
                let safety = declaration
                    .interface()
                    .effects()
                    .function_attributes()
                    .safety;
                let (target, source, outer_type) = if matches!(
                    declaration.interface().owner(),
                    hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::GenericTemplate(
                        _
                    ))
                ) {
                    let template =
                        state.request_imported_constructor_template(*declaration.clone())?;
                    let (loaded, _) = ImportedGenericTarget::Constructor(template).signature(state);
                    (
                        ConstructorReferenceTarget::Imported(template),
                        loaded.signature,
                        loaded.receiver,
                    )
                } else {
                    let candidate = ImportedCallableCandidate::Declaration(declaration.clone());
                    let signature = state
                        .imported_native_signature(&candidate)
                        .map_err(|error| error.diagnostic("constructor reference signature"))?;
                    (
                        ConstructorReferenceTarget::Dependency(declaration.clone()),
                        signature,
                        declaration
                            .interface()
                            .receiver()
                            .map(|ty| state.imported_signature_type(ty))
                            .transpose()
                            .map_err(|error| error.diagnostic("constructor outer type"))?,
                    )
                };
                (
                    target,
                    DeclarationSignature {
                        owner_parameters: source.owner_parameters,
                        callable_parameters: source.callable_parameters,
                        value_parameters: source
                            .value_parameters
                            .into_iter()
                            .map(|parameter| ValueParameter {
                                name: parameter.name,
                                ty: parameter.ty,
                                calling: ValueParameterCalling::Required,
                            })
                            .collect(),
                        return_type: source.return_type,
                    },
                    safety,
                    outer_type,
                )
            }
        };
        let mut enclosing = if let Some(formal) = outer_type {
            let required = state
                .nominal_application(formal)
                .expect("an inner constructor has a nominal outer type")
                .template;
            let actual = match &candidate.enclosing {
                Some(receiver) => state.enclosing_receiver_application(receiver.clone(), required),
                None => state.implicit_enclosing_instance(required, span),
            }
            .ok_or_else(|| {
                "inner constructor reference requires its declaring outer instance".to_owned()
            })?;
            Some((actual, formal))
        } else {
            None
        };
        let fixed_arguments = candidate.fixed.map(|ty| {
            state
                .nominal_application(ty)
                .expect("a fixed constructor alias has a nominal application")
                .arguments
        });
        if let Some(arguments) = &fixed_arguments {
            let bindings = signature
                .owner_parameters
                .iter()
                .map(|parameter| parameter.id)
                .zip(arguments.iter().copied())
                .collect::<Vec<_>>();
            for parameter in &mut signature.value_parameters {
                parameter.ty = state.instantiate_method_ty(parameter.ty, &bindings);
            }
            signature.return_type = state.instantiate_method_ty(signature.return_type, &bindings);
            if let Some((_, formal)) = &mut enclosing {
                *formal = state.instantiate_method_ty(*formal, &bindings);
            }
            signature.owner_parameters.clear();
        }
        let (alias_arguments, owner_arguments) = if let Some(alias) = &candidate.alias {
            let alias = state
                .reload_alias_template(alias)
                .expect("an applicable constructor alias retains its declaration");
            let mapped = state
                .alias_constructor_signature(&alias, &signature, &[], span)
                .ok_or_else(|| {
                    "typealias constructor requires a complete host type application".to_owned()
                })?;
            if let Some((_, formal)) = &mut enclosing {
                *formal = state.instantiate_method_ty(*formal, &mapped.target_bindings);
            }
            signature = mapped.signature;
            let fixed_count = alias
                .arguments
                .as_ref()
                .map_or(alias.outer_parameter_count, Vec::len);
            let own = signature.callable_parameters.split_off(fixed_count);
            signature.owner_parameters = std::mem::replace(&mut signature.callable_parameters, own);
            let owners = mapped.explicit[..fixed_count]
                .iter()
                .map(|argument| match argument {
                    ResolvedCallTypeArgument::Explicit { ty, .. } => *ty,
                    ResolvedCallTypeArgument::Infer { .. } => {
                        unreachable!("captured alias parameters are fixed by the lexical scope")
                    }
                })
                .collect();
            (
                Some(
                    mapped
                        .target_bindings
                        .into_iter()
                        .map(|(_, ty)| ty)
                        .collect(),
                ),
                owners,
            )
        } else {
            let owners = if fixed_arguments.is_none() {
                enclosing.as_ref().map_or_else(Vec::new, |(actual, _)| {
                    state
                        .nominal_application(actual.ty)
                        .expect("an outer application")
                        .arguments
                })
            } else {
                Vec::new()
            };
            signature
                .callable_parameters
                .extend(signature.owner_parameters.split_off(owners.len()));
            (None, owners)
        };
        let attributes = hir::FunctionAttributes {
            safety,
            ..Default::default()
        };
        Ok(Self {
            candidate,
            target,
            signature,
            effects: CallableEffects {
                is_suspend: false,
                attributes,
            },
            fixed_arguments,
            alias_arguments,
            owner_arguments,
            enclosing,
            span,
        })
    }
}
