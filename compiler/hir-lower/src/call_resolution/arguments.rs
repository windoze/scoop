//! Candidate-specific source-to-parameter mapping.

use super::candidates::{CallableView, NominalConstructorView, ReceiverShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourceInputId(u32);

impl SourceInputId {
    pub(crate) fn from_index(index: usize) -> Self {
        Self(u32::try_from(index).expect("source argument index exceeds u32"))
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    #[cfg(test)]
    pub(super) fn from_test_index(index: usize) -> Self {
        Self::from_index(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ValueParameterId(u32);

impl ValueParameterId {
    fn from_index(index: usize) -> Self {
        Self(u32::try_from(index).expect("value parameter index exceeds u32"))
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiverInput {
    Absent,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParameterInput {
    pub(crate) parameter: ValueParameterId,
    pub(crate) input: SourceInputId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateArgumentMap {
    pub(crate) receiver: ReceiverInput,
    pub(crate) parameters: Vec<ParameterInput>,
    pub(crate) source_order: Vec<SourceInputId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArityMismatch {
    pub(crate) expected: usize,
    pub(crate) supplied: usize,
}

impl CandidateArgumentMap {
    /// M16 has exact positional calls, but still gives every candidate an
    /// independent semantic map. M17 can replace this constructor without
    /// changing the constraint solver that consumes the result.
    pub(crate) fn exact(view: &CallableView, supplied: usize) -> Result<Self, ArityMismatch> {
        let expected = view.value_parameters.len();
        if expected != supplied {
            return Err(ArityMismatch { expected, supplied });
        }
        Ok(Self::positional(
            expected,
            match view.receiver {
                ReceiverShape::None | ReceiverShape::Instance => ReceiverInput::Absent,
                ReceiverShape::Extension(_) => ReceiverInput::Present,
            },
        ))
    }

    pub(crate) fn exact_nominal(
        view: &NominalConstructorView,
        supplied: usize,
    ) -> Result<Self, ArityMismatch> {
        let expected = view.value_parameters.len();
        if expected != supplied {
            return Err(ArityMismatch { expected, supplied });
        }
        Ok(Self::positional(expected, ReceiverInput::Absent))
    }

    /// Build the positional prefix selected by a constructor after its own
    /// required/default arity rules have accepted the source call.
    pub(crate) fn positional(parameter_count: usize, receiver: ReceiverInput) -> Self {
        let supplied = parameter_count;
        let source_order = (0..supplied).map(SourceInputId::from_index).collect();
        let parameters = (0..supplied)
            .map(|index| ParameterInput {
                parameter: ValueParameterId::from_index(index),
                input: SourceInputId::from_index(index),
            })
            .collect();
        Self {
            receiver,
            parameters,
            source_order,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_resolution::candidates::{
        CallableEffects, CallableSource, NominalConstructorSource, SourceDispatch, ValueParameter,
    };
    use scoop_ast::Span;
    use scoop_hir as hir;

    fn view(parameter_count: usize, receiver: ReceiverShape) -> CallableView {
        CallableView {
            target: CallableSource::Free(hir::FunctionId::from_raw(0_u32.into())),
            receiver,
            owner_parameters: Vec::new(),
            callable_parameters: Vec::new(),
            value_parameters: (0..parameter_count)
                .map(|index| ValueParameter {
                    name: format!("p{index}"),
                    ty: hir::TypeId::from_raw((index as u32).into()),
                })
                .collect(),
            return_type: hir::TypeId::from_raw(0_u32.into()),
            effects: CallableEffects {
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
            },
            dispatch: SourceDispatch::Direct,
            declaration_span: Span::new(0, 0),
        }
    }

    #[test]
    fn exact_mapping_is_candidate_owned_and_preserves_source_order() {
        let map = CandidateArgumentMap::exact(
            &view(
                2,
                ReceiverShape::Extension(hir::TypeId::from_raw(3_u32.into())),
            ),
            2,
        )
        .expect("matching positional arguments");
        assert_eq!(map.receiver, ReceiverInput::Present);
        assert_eq!(map.parameters[0].parameter.index(), 0);
        assert_eq!(map.parameters[1].input.index(), 1);
        assert_eq!(
            map.source_order
                .iter()
                .map(|input| input.index())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn exact_mapping_reports_candidate_arity() {
        assert_eq!(
            CandidateArgumentMap::exact(&view(2, ReceiverShape::None), 1),
            Err(ArityMismatch {
                expected: 2,
                supplied: 1,
            })
        );
    }

    #[test]
    fn exact_nominal_mapping_uses_constructor_fields() {
        let view = NominalConstructorView {
            target: NominalConstructorSource::Struct(hir::StructId::from_raw(0_u32.into())),
            owner_parameters: Vec::new(),
            value_parameters: vec![
                ValueParameter {
                    name: "left".to_string(),
                    ty: hir::TypeId::from_raw(0_u32.into()),
                },
                ValueParameter {
                    name: "right".to_string(),
                    ty: hir::TypeId::from_raw(0_u32.into()),
                },
            ],
            result_type: hir::TypeId::from_raw(0_u32.into()),
            declaration_span: Span::new(0, 0),
        };

        let mapping = CandidateArgumentMap::exact_nominal(&view, 2).expect("matching arity");
        assert_eq!(mapping.parameters[0].input.index(), 0);
        assert_eq!(mapping.parameters[1].parameter.index(), 1);
        assert_eq!(
            CandidateArgumentMap::exact_nominal(&view, 1),
            Err(ArityMismatch {
                expected: 2,
                supplied: 1,
            })
        );
    }
}
