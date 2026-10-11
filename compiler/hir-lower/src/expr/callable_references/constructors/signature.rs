use super::*;

impl ConstructorReferenceDeclaration {
    pub(super) fn resolve(
        state: &mut Lowerer,
        candidate: ConstructorReferenceCandidate,
        span: Span,
    ) -> Result<Self, String> {
        let (target, mut signature, safety) = match &candidate.source {
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
                )
            }
            ConstructorReferenceSource::Imported(declaration) => {
                let safety = declaration
                    .interface()
                    .effects()
                    .function_attributes()
                    .safety;
                let (target, source) = if matches!(
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
                    )
                } else {
                    let candidate = ImportedCallableCandidate::Declaration(declaration.clone());
                    let signature = state
                        .imported_native_signature(&candidate)
                        .map_err(|error| error.diagnostic("constructor reference signature"))?;
                    (
                        ConstructorReferenceTarget::Dependency(declaration.clone()),
                        signature,
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
                )
            }
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
            // Constructor host parameters are inferred at the reference creation site.
            signature
                .callable_parameters
                .append(&mut signature.owner_parameters);
            (None, Vec::new())
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
            span,
        })
    }
}
