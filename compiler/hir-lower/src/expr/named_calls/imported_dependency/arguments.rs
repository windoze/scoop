//! Source-call argument mapping for imported dependency callables.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::SignatureTypeKey;

use crate::call_resolution::arguments::ArgumentShapeFailure;

#[derive(Clone, Debug)]
pub(in crate::expr) struct ImportedArgumentMap {
    parameters: Vec<ImportedParameterInput>,
    source_parameters: Vec<SignatureTypeKey>,
    defaults: usize,
    vararg: bool,
}

#[derive(Clone, Debug)]
pub(super) enum ImportedParameterInput {
    Explicit(usize),
    Default(hir::ExportDefaultTemplateKeyV1),
    Vararg,
}

impl ImportedArgumentMap {
    pub(super) fn source(
        parameters: &[hir::CallableSourceParameterV1],
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        let positional_required_call = arguments
            .iter()
            .all(|argument| matches!(argument.name, ast::CallArgumentName::Positional))
            && parameters.iter().all(|parameter| {
                matches!(
                    parameter.calling(),
                    hir::CallableParameterCallingV1::Required
                )
            });
        let mut mapped = vec![None; parameters.len()];
        let mut source_parameters = vec![None; arguments.len()];
        let mut next = 0;
        let mut named_only = false;

        for (source_index, argument) in arguments.iter().enumerate() {
            match &argument.name {
                ast::CallArgumentName::Positional => {
                    if named_only {
                        return Err(ArgumentShapeFailure::PositionalAfterNamed);
                    }
                    let Some(parameter) = parameters.get(next) else {
                        return Err(ArgumentShapeFailure::Arity {
                            expected: parameters.len(),
                            supplied: arguments.len(),
                        });
                    };
                    match parameter.calling() {
                        hir::CallableParameterCallingV1::Required
                        | hir::CallableParameterCallingV1::Default { .. } => {
                            if matches!(argument.spread, ast::SpreadSyntax::Spread(_)) {
                                return Err(ArgumentShapeFailure::SpreadForRegular {
                                    name: parameter.name().as_str().to_owned(),
                                });
                            }
                            mapped[next] = Some(ImportedParameterInput::Explicit(source_index));
                            source_parameters[source_index] = Some(parameter.value_type().clone());
                            next += 1;
                        }
                        hir::CallableParameterCallingV1::VarargEmpty { element_type }
                        | hir::CallableParameterCallingV1::VarargDefault { element_type, .. } => {
                            mapped[next] = Some(ImportedParameterInput::Vararg);
                            source_parameters[source_index] = Some(match argument.spread {
                                ast::SpreadSyntax::Plain => element_type.clone(),
                                ast::SpreadSyntax::Spread(_) => parameter.value_type().clone(),
                            });
                        }
                    }
                }
                ast::CallArgumentName::Named(name) => {
                    let Some(index) = parameters
                        .iter()
                        .position(|parameter| parameter.name().as_str() == name.text)
                    else {
                        return Err(ArgumentShapeFailure::UnknownName {
                            name: name.text.clone(),
                        });
                    };
                    let parameter = &parameters[index];
                    if mapped[index].is_some() {
                        return Err(ArgumentShapeFailure::DuplicateParameter {
                            name: parameter.name().as_str().to_owned(),
                        });
                    }
                    match parameter.calling() {
                        hir::CallableParameterCallingV1::Required
                        | hir::CallableParameterCallingV1::Default { .. } => {
                            if matches!(argument.spread, ast::SpreadSyntax::Spread(_)) {
                                return Err(ArgumentShapeFailure::SpreadForRegular {
                                    name: parameter.name().as_str().to_owned(),
                                });
                            }
                            mapped[index] = Some(ImportedParameterInput::Explicit(source_index));
                            source_parameters[source_index] = Some(parameter.value_type().clone());
                        }
                        hir::CallableParameterCallingV1::VarargEmpty { .. }
                        | hir::CallableParameterCallingV1::VarargDefault { .. } => {
                            mapped[index] = Some(ImportedParameterInput::Vararg);
                            source_parameters[source_index] = Some(parameter.value_type().clone());
                            named_only = true;
                        }
                    }
                    if index == next {
                        next += 1;
                        while next < mapped.len() && mapped[next].is_some() {
                            next += 1;
                        }
                    } else {
                        named_only = true;
                    }
                }
            }
        }

        if positional_required_call && arguments.len() != parameters.len() {
            return Err(ArgumentShapeFailure::Arity {
                expected: parameters.len(),
                supplied: arguments.len(),
            });
        }

        let mut defaults = 0;
        let mut vararg = false;
        let mut resolved = Vec::with_capacity(parameters.len());
        for (parameter, input) in parameters.iter().zip(mapped) {
            let input = match input {
                Some(input) => input,
                None => match parameter.calling() {
                    hir::CallableParameterCallingV1::Required => {
                        return Err(ArgumentShapeFailure::MissingRequired {
                            name: parameter.name().as_str().to_owned(),
                        });
                    }
                    hir::CallableParameterCallingV1::Default { template } => {
                        defaults += 1;
                        ImportedParameterInput::Default(*template)
                    }
                    hir::CallableParameterCallingV1::VarargEmpty { .. } => {
                        vararg = true;
                        ImportedParameterInput::Vararg
                    }
                    hir::CallableParameterCallingV1::VarargDefault { .. } => {
                        defaults += 1;
                        vararg = true;
                        ImportedParameterInput::Vararg
                    }
                },
            };
            vararg |= matches!(input, ImportedParameterInput::Vararg);
            resolved.push(input);
        }

        Ok(Self {
            parameters: resolved,
            source_parameters: source_parameters
                .into_iter()
                .map(|parameter| parameter.expect("each source argument is bound exactly once"))
                .collect(),
            defaults,
            vararg,
        })
    }

    pub(super) fn parameters(&self) -> &[ImportedParameterInput] {
        &self.parameters
    }

    pub(super) fn source_parameters(&self) -> &[SignatureTypeKey] {
        &self.source_parameters
    }

    pub(super) const fn defaults(&self) -> usize {
        self.defaults
    }

    pub(in crate::expr) const fn has_vararg(&self) -> bool {
        self.vararg
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DefinitionOrigin,
        NormalizedSourcePath, SourceContextKey, SourceIdentity, SourceSpan,
    };

    use super::*;

    #[test]
    fn named_arguments_reorder_into_declaration_parameters() {
        let parameters = [required_parameter("first"), required_parameter("second")];
        let arguments = [named_argument("second"), named_argument("first")];

        let mapping = ImportedArgumentMap::source(&parameters, &arguments).unwrap();

        assert!(matches!(
            mapping.parameters(),
            [
                ImportedParameterInput::Explicit(1),
                ImportedParameterInput::Explicit(0)
            ]
        ));
        assert_eq!(mapping.source_parameters(), &[unit_type(), unit_type()]);
    }

    fn required_parameter(name: &str) -> hir::CallableSourceParameterV1 {
        hir::CallableSourceParameterV1::new(
            CanonicalIdentifier::new(name).unwrap(),
            unit_type(),
            hir::CallableParameterCallingV1::Required,
            definition_origin(),
        )
    }

    fn named_argument(name: &str) -> ast::CallArgument {
        let span = ast::Span::new(0, 0);
        ast::CallArgument {
            name: ast::CallArgumentName::Named(ast::Ident {
                text: name.to_owned(),
                span,
            }),
            spread: ast::SpreadSyntax::Plain,
            expression: ast::Expr::UnitLiteral { span },
            span,
        }
    }

    fn unit_type() -> SignatureTypeKey {
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
    }

    fn definition_origin() -> hir::ExportDefinitionSourceV1 {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Test.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        hir::ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap(),
        )
    }
}
