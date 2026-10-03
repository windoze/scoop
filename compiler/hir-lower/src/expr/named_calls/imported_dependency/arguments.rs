//! Checked declaration views use the common source-to-parameter mapping.

use crate::call_resolution::arguments::{
    ArgumentShape, ArgumentShapeFailure, CandidateArgumentMap, ParameterInput, ReceiverInput,
};
use crate::call_resolution::candidates::{ArgumentMode, ValueParameter};
use scoop_hir as hir;

#[derive(Clone, Debug)]
pub(in crate::expr) struct ImportedArgumentMap {
    mapping: CandidateArgumentMap<hir::ExportDefaultTemplateKeyV1>,
    vararg: bool,
}

impl ImportedArgumentMap {
    pub(super) fn map(
        parameters: &[ValueParameter<hir::ExportDefaultTemplateKeyV1>],
        arguments: &[ArgumentShape<'_>],
        mode: ArgumentMode,
        operator_set: bool,
    ) -> Result<Self, ArgumentShapeFailure> {
        let mapping = CandidateArgumentMap::declaration(
            parameters,
            mode,
            arguments,
            ReceiverInput::Absent,
            operator_set,
        )?;
        Ok(Self {
            mapping,
            vararg: parameters.iter().any(ValueParameter::is_vararg),
        })
    }

    pub(super) fn mapping(&self) -> &CandidateArgumentMap<hir::ExportDefaultTemplateKeyV1> {
        &self.mapping
    }

    pub(super) fn parameters(&self) -> &[ParameterInput<hir::ExportDefaultTemplateKeyV1>] {
        &self.mapping.parameters
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
    use super::*;
    use crate::call_resolution::arguments::ResolvedParameterInput;
    use crate::call_resolution::candidates::ValueParameterCalling;

    #[test]
    fn named_arguments_reorder_into_declaration_parameters() {
        let types = [0_u32, 1].map(|index| hir::TypeId::from_raw(index.into()));
        let parameters = ["first", "second"]
            .into_iter()
            .zip(types)
            .map(|(name, ty)| ValueParameter {
                name: name.into(),
                calling: ValueParameterCalling::Required,
                ty,
            })
            .collect::<Vec<_>>();
        let arguments = ["second", "first"].map(|name| ArgumentShape {
            name: Some(name),
            spread: false,
        });
        let mapping =
            ImportedArgumentMap::map(&parameters, &arguments, ArgumentMode::Mixed, false).unwrap();
        let inputs = mapping
            .parameters()
            .iter()
            .map(|parameter| match parameter.input {
                ResolvedParameterInput::Explicit(source) => source.index(),
                _ => panic!("required parameters retain their explicit inputs"),
            })
            .collect::<Vec<_>>();
        assert_eq!(inputs, vec![1, 0]);
        assert_eq!(
            mapping.mapping().forwarding_parameter_types(&parameters),
            vec![types[1], types[0]]
        );
    }
}
