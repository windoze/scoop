use super::*;
use crate::imports::{ImportLookupLayer, lookup::TypeLookupTarget};
use crate::namespace::TopLevelTypeTarget;
use hir::{Function, FunctionKind};
use la_arena::Arena;

impl Lowerer {
    pub(in crate::class) fn coding_candidate_declared(
        &mut self,
        name: &str,
        parameter: TypeId,
        candidates: &[crate::CallableCandidate],
    ) -> bool {
        for candidate in candidates {
            let function = &self.functions[candidate.function];
            if function.name.rsplit('.').next() != Some(name)
                || function.method_type_param_count() != 0
            {
                continue;
            }
            let crate::CallableCandidateOwner::Method(application) = candidate.owner else {
                unreachable!("nominal method candidate")
            };
            let arguments = self.method_owner_arguments(application).to_vec();
            let candidate = self.instantiated_signature(candidate.function, &arguments, &[]);
            if candidate.params.len() == 1 && self.types_equal(candidate.params[0].ty, parameter) {
                return true;
            }
        }
        false
    }

    pub(in crate::class) fn register_coding_method(
        &mut self,
        owner: Owner,
        name: &str,
        parameter_name: &str,
        parameter: TypeId,
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
                params: vec![crate::FnParam {
                    name: ast::Ident {
                        text: parameter_name.into(),
                        span,
                    },
                    ty: parameter,
                    calling: crate::FnParamCalling::Required,
                }],
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
