//! Dependency declaration storage adapts to the common argument mapper.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::SignatureTypeKey;

use crate::call_resolution::arguments::{
    ArgumentShape, ArgumentShapeFailure, CandidateArgumentMap, ParameterCalling, ParameterInput,
    ParameterShape, ReceiverInput, SourceInputId, SourceInputKind,
};
use crate::call_resolution::candidates::ArgumentMode;

#[derive(Clone, Debug)]
pub(in crate::expr) struct ImportedArgumentMap {
    mapping: CandidateArgumentMap<hir::ExportDefaultTemplateKeyV1>,
    source_parameters: Vec<SignatureTypeKey>,
    vararg: bool,
}

impl ImportedArgumentMap {
    pub(super) fn source(
        parameters: &[hir::CallableSourceParameterV1],
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            parameters,
            &arguments
                .iter()
                .map(ArgumentShape::from)
                .collect::<Vec<_>>(),
            false,
        )
    }

    pub(super) fn lowered(
        parameters: &[hir::CallableSourceParameterV1],
        count: usize,
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            parameters,
            &vec![
                ArgumentShape {
                    name: None,
                    spread: false
                };
                count
            ],
            false,
        )
    }

    pub(super) fn source_operator_set(
        parameters: &[hir::CallableSourceParameterV1],
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            parameters,
            &arguments
                .iter()
                .map(ArgumentShape::from)
                .collect::<Vec<_>>(),
            true,
        )
    }

    fn map(
        parameters: &[hir::CallableSourceParameterV1],
        arguments: &[ArgumentShape<'_>],
        operator_set: bool,
    ) -> Result<Self, ArgumentShapeFailure> {
        let shapes = parameters
            .iter()
            .map(|parameter| ParameterShape {
                name: parameter.name().as_str(),
                calling: match parameter.calling() {
                    hir::CallableParameterCallingV1::Required => ParameterCalling::Required,
                    hir::CallableParameterCallingV1::Default { template } => {
                        ParameterCalling::Default(*template)
                    }
                    hir::CallableParameterCallingV1::VarargEmpty { .. } => {
                        ParameterCalling::Vararg { default: None }
                    }
                    hir::CallableParameterCallingV1::VarargDefault { template, .. } => {
                        ParameterCalling::Vararg {
                            default: Some(*template),
                        }
                    }
                },
            })
            .collect::<Vec<_>>();
        let mapping = if operator_set {
            CandidateArgumentMap::operator_set(&shapes, arguments, ReceiverInput::Absent)?
        } else {
            CandidateArgumentMap::map(
                &shapes,
                ArgumentMode::Mixed,
                arguments,
                ReceiverInput::Absent,
            )?
        };
        let source_parameters = mapping
            .source_order
            .iter()
            .map(|source| {
                let (parameter, kind) = mapping.source_binding(*source);
                let parameter = &parameters[parameter.index()];
                match (parameter.calling(), kind) {
                    (
                        hir::CallableParameterCallingV1::VarargEmpty { element_type }
                        | hir::CallableParameterCallingV1::VarargDefault { element_type, .. },
                        SourceInputKind::VarargElement,
                    ) => element_type.clone(),
                    _ => parameter.value_type().clone(),
                }
            })
            .collect();
        let vararg = shapes
            .iter()
            .any(|parameter| matches!(parameter.calling, ParameterCalling::Vararg { .. }));
        Ok(Self {
            mapping,
            source_parameters,
            vararg,
        })
    }

    pub(super) fn parameters(&self) -> &[ParameterInput<hir::ExportDefaultTemplateKeyV1>] {
        &self.mapping.parameters
    }

    pub(super) fn source_parameters(&self) -> &[SignatureTypeKey] {
        &self.source_parameters
    }

    pub(super) fn is_array_input(&self, source: usize) -> bool {
        self.mapping
            .source_binding(SourceInputId::from_index(source))
            .1
            == SourceInputKind::VarargArray
    }

    pub(super) fn defaults(&self) -> usize {
        self.mapping.explicit_default_count()
    }

    pub(in crate::expr) const fn has_vararg(&self) -> bool {
        self.vararg
    }
}

#[cfg(test)]
mod tests {
    use crate::call_resolution::arguments::ResolvedParameterInput;
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

        let inputs = mapping
            .parameters()
            .iter()
            .map(|parameter| match &parameter.input {
                ResolvedParameterInput::Explicit(source) => source.index(),
                _ => panic!("required parameters retain their explicit inputs"),
            })
            .collect::<Vec<_>>();
        assert_eq!(inputs, vec![1, 0]);
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
