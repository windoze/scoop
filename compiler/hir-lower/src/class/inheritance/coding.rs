use super::*;
use crate::imports::{ImportLookupLayer, lookup::TypeLookupTarget};
use crate::namespace::TopLevelTypeTarget;
use ast::Span;
use hir::{Function, FunctionKind};
use la_arena::Arena;

mod annotations;
mod codecs;
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
}

impl CodingDirection {
    fn method(self) -> &'static str {
        match self {
            Self::Encode => "encode",
            Self::Decode => "decode",
        }
    }
    fn factory(self) -> &'static str {
        match self {
            Self::Encode => "encoder",
            Self::Decode => "decoder",
        }
    }
    fn protocol(self) -> &'static str {
        match self {
            Self::Encode => "Encodable",
            Self::Decode => "Decodable",
        }
    }
    fn stream(self) -> &'static str {
        match self {
            Self::Encode => "Encoder",
            Self::Decode => "Decoder",
        }
    }
    fn unit_codec(self) -> &'static str {
        match self {
            Self::Encode => "UnitEncoder",
            Self::Decode => "UnitDecoder",
        }
    }
    fn function_codec(self) -> &'static str {
        match self {
            Self::Encode => "EncodeFunction",
            Self::Decode => "DecodeFunction",
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
            if function.name.rsplit('.').next() != Some(name) {
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
        let host_ty = self.owner_ty(owner);
        let parameters = self.owner_type_params(owner);
        let access = self.member_access(
            ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Public,
                span,
            },
            span,
            "method",
            owner,
            self.current_file,
            crate::visibility::MemberSlotAccess::Override,
        );
        let function = self.functions.alloc(Function {
            signature: hir::CallableSignature {
                context_parameters: Vec::new(),
                release_callability: Default::default(),
                name: format!("{}.{}", owner.describe_name(self), name),
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty: result,
                attributes: hir::FunctionAttributes::default(),
                span,
            },
            access,
            genericity: hir::FunctionGenericity::Plain,
            kind: FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner: host_ty,
                modifier: hir::MethodModifier::Final,
                dispatch: hir::MethodDispatch::Direct,
            }),
        });
        self.register_method_parameters(function, parameters.clone(), Vec::new());
        self.source_function_declarations.insert(
            function,
            crate::SourceFunctionDeclaration { name: name.into() },
        );
        self.function_owner.insert(function, owner);
        self.function_files.insert(function, self.current_file);
        self.signatures.insert(
            function,
            crate::FnSig {
                context_parameters: Vec::new(),
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                attributes: hir::FunctionAttributes::default(),
                owner_type_param_count: parameters.len(),
                type_params: parameters,
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
            },
        );
        match owner {
            Owner::Struct(id) => self.structs[id].methods.push(function),
            Owner::Enum(id) => self.enums[id].methods.push(function),
            Owner::Class(id) => self.classes[id].methods.push(function),
            Owner::Object(id) => self.classes[self.objects[id].backing_class]
                .methods
                .push(function),
            Owner::Interface(_) => unreachable!("interfaces do not request nominal bodies"),
        }
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
