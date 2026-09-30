//! Calling shape is available before any default expression is prepared.
use super::*;
use crate::FnVarargOmission;

impl Lowerer {
    pub(crate) fn source_parameter_calling(
        &self,
        owner: SourceParameterOwner,
        index: usize,
        value_type: hir::TypeId,
        calling: &FnParamCalling,
    ) -> SourceParameterCalling {
        let key = SourceDefaultKey::new(owner, index);
        let inherited = matches!(owner, SourceParameterOwner::Function(function) if self.function_parameter_has_default(function, index));
        let existing = self.default_templates.get(&key.tuple()).copied();
        let source = existing.map_or(
            DefaultArgumentSource::Parameter(key),
            DefaultArgumentSource::Ready,
        );
        match calling {
            FnParamCalling::Required if existing.is_some() || inherited => {
                SourceParameterCalling::Default(source)
            }
            FnParamCalling::Required => SourceParameterCalling::Required,
            FnParamCalling::Default { .. } => SourceParameterCalling::Default(source),
            FnParamCalling::Vararg {
                element_ty,
                omission,
            } => SourceParameterCalling::Vararg {
                element_type: *element_ty,
                array_type: value_type,
                omission: match omission {
                    FnVarargOmission::Default { .. } => SourceVarargOmission::Default(source),
                    FnVarargOmission::EmptyArray if existing.is_some() || inherited => {
                        SourceVarargOmission::Default(source)
                    }
                    FnVarargOmission::EmptyArray => SourceVarargOmission::EmptyArray,
                },
            },
        }
    }

    pub(super) fn function_parameter_has_default(
        &self,
        function: hir::FunctionId,
        index: usize,
    ) -> bool {
        let mut pending = vec![function];
        let mut visited = std::collections::HashSet::new();
        while let Some(function) = pending.pop() {
            if !visited.insert(function) {
                continue;
            }
            if self
                .default_templates
                .contains_key(&(SourceParameterOwner::Function(function), index as u32))
            {
                return true;
            }
            if self
                .signatures
                .get(&function)
                .and_then(|signature| signature.params.get(index))
                .is_some_and(|parameter| {
                    matches!(
                        parameter.calling,
                        FnParamCalling::Default { .. }
                            | FnParamCalling::Vararg {
                                omission: FnVarargOmission::Default { .. },
                                ..
                            }
                    )
                })
            {
                return true;
            }
            for source in self
                .override_default_sources
                .get(&function)
                .into_iter()
                .flatten()
            {
                match source {
                    DefaultOverrideSource::Local { function, .. } => pending.push(*function),
                    DefaultOverrideSource::Imported { declaration, .. } => {
                        if self
                            .dependencies
                            .as_ref()
                            .and_then(|dependencies| {
                                dependencies.callable_source_interface(*declaration)
                            })
                            .and_then(|source| source.parameters().parameters().get(index))
                            .is_some_and(|parameter| parameter.calling().template().is_some())
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}
