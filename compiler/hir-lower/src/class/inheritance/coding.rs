use super::*;
use crate::imports::{ImportLookupLayer, lookup::TypeLookupTarget};
use crate::namespace::TopLevelTypeTarget;
use ast::Span;

mod annotations;
mod codecs;
mod defaults;
mod expressions;
mod tuple;
pub(super) use codecs::SelectedCodec;

#[derive(Clone, Copy)]
pub(super) struct CodingContext {
    pub owner: Owner,
    pub target: TypeId,
    pub interface: hir::SourceNominalId,
    pub direction: CodingDirection,
}

#[derive(Clone, Copy)]
pub(super) enum CodingDirection {
    Encode,
    Decode,
    Inspect(super::inspection::InspectionTypeFactory),
}

impl CodingDirection {
    fn method(self) -> &'static str {
        match self {
            Self::Encode => "encode",
            Self::Decode => "decode",
            Self::Inspect(_) => "inspect",
        }
    }
    fn factory(self) -> &'static str {
        match self {
            Self::Encode => "encoder",
            Self::Decode => "decoder",
            Self::Inspect(_) => "inspector",
        }
    }
    fn protocol(self) -> &'static str {
        match self {
            Self::Encode => "ValueEncoder",
            Self::Decode => "ValueDecoder",
            Self::Inspect(_) => "ValueInspector",
        }
    }
    fn stream(self) -> &'static str {
        match self {
            Self::Encode => "Encoder",
            Self::Decode => "Decoder",
            Self::Inspect(_) => "ValueView",
        }
    }
    fn unit_codec(self) -> &'static str {
        match self {
            Self::Encode => "UnitEncoder",
            Self::Decode => "UnitDecoder",
            Self::Inspect(_) => "UnitInspector",
        }
    }
    fn function_codec(self) -> &'static str {
        match self {
            Self::Encode => "ValueEncoderFunction",
            Self::Decode => "ValueDecoderFunction",
            Self::Inspect(_) => "ValueInspectorFunction",
        }
    }
}

impl Lowerer {
    pub(in crate::class) fn coding_candidate_declared(
        &mut self,
        name: &str,
        parameters: &[TypeId],
        candidates: &[crate::CallableCandidate],
    ) -> bool {
        for candidate in candidates {
            let function = &self.functions[candidate.function];
            if self.source_function_name(candidate.function) != name {
                continue;
            }
            if self.invalid_override_methods.contains(&candidate.function) {
                return true;
            }
            if function.method_type_param_count() != 0 {
                continue;
            }
            let crate::CallableCandidateOwner::Method(application) = candidate.owner else {
                unreachable!("nominal method candidate")
            };
            let arguments = self.method_owner_arguments(application).to_vec();
            let candidate = self.instantiated_signature(candidate.function, &arguments, &[]);
            if candidate.params.len() == parameters.len()
                && candidate
                    .params
                    .iter()
                    .zip(parameters)
                    .all(|(parameter, &required)| self.types_equal(parameter.ty, required))
            {
                return true;
            }
        }
        false
    }

    pub(in crate::class) fn register_coding_method(
        &mut self,
        owner: Owner,
        name: &str,
        values: &[(&str, TypeId)],
        result: TypeId,
    ) -> FunctionId {
        let span = match owner {
            Owner::Struct(id) => self.structs[id].span,
            Owner::Enum(id) => self.enums[id].span,
            Owner::Class(id) => self.classes[id].span,
            Owner::Object(id) => self.objects[id].span,
            Owner::Interface(_) => unreachable!("interfaces do not request nominal bodies"),
        };
        let signature = crate::FnSig {
            result_pending: false,
            parameter_pending: false,
            context_parameters: Vec::new(),
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            attributes: hir::FunctionAttributes::default(),
            owner_type_param_count: 0,
            type_params: Vec::new(),
            params: values
                .iter()
                .map(|&(name, ty)| crate::FnParam {
                    name: ast::Ident {
                        text: name.into(),
                        span,
                    },
                    ty,
                    calling: crate::FnParamCalling::Required,
                })
                .collect(),
            return_ty: result,
        };
        let function = self.allocate_synthesized_method(
            owner,
            name,
            signature,
            hir::MethodModifier::Final,
            ast::DeclaredVisibility::Public,
            span,
        );
        self.source_function_declarations.insert(
            function,
            crate::SourceFunctionDeclaration {
                name: name.into(),
                name_span: None,
            },
        );
        function
    }

    pub(crate) fn core_coding_nominal(&self, name: &str) -> Option<hir::SourceNominalId> {
        self.type_lookup_layers(name).into_iter().find_map(|layer| {
            layer
                .candidates
                .into_iter()
                .find_map(|candidate| match candidate.target {
                    TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target)) => {
                        let identity = self.nominal_identity(target.owner());
                        identity
                            .source()
                            .filter(|source| {
                                source.declaration().origin() == scoop_identity::ConeIdentity::CORE
                            })
                            .map(|_| identity.declaration_id())
                    }
                    TypeLookupTarget::Dependency(binding)
                        if layer.kind == ImportLookupLayer::CorePrelude =>
                    {
                        binding.target().source_nominal()
                    }
                    _ => None,
                })
        })
    }
}
