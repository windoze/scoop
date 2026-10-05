use super::*;
use crate::imports::{ImportLookupLayer, lookup::TypeLookupTarget};
use crate::namespace::TopLevelTypeTarget;

impl Lowerer {
    pub(in crate::class) fn derive_missing_encoding(
        &mut self,
        owner: Owner,
        member: &InterfaceMemberInstance,
        interfaces: &[TypeId],
        candidates: &[crate::CallableCandidate],
    ) -> Option<FunctionId> {
        let encodable = self.core_encoding_nominal("Encodable")?;
        let encoder = self.core_encoding_nominal("Encoder")?;
        let encodable_ty = interfaces.iter().copied().find(|ty| matches!(self.types[*ty], Type::Interface(application) if self.interface_applications[application].template == encodable))?;
        let signature = &member.signature;
        if signature.name != "encode"
            || signature.parameters.len() != 1
            || signature.result != self.unit
            || signature.is_suspend
            || !signature.context_parameters.is_empty()
            || !matches!(self.types[signature.parameters[0]], Type::Interface(application) if self.interface_applications[application].template == encoder)
        {
            return None;
        }
        // An invalid explicit declaration must keep its normal diagnostic;
        // synthesis cannot add a duplicate parameter signature beside it.
        for candidate in candidates {
            let function = &self.functions[candidate.function];
            // This spelling selects a member; identity remains its FunctionId.
            if function.name.rsplit('.').next() != Some("encode")
                || function.method_type_param_count() != 0
            {
                continue;
            }
            let crate::CallableCandidateOwner::Method(application) = candidate.owner else {
                unreachable!("nominal method candidate")
            };
            let arguments = self.method_owner_arguments(application).to_vec();
            let candidate = self.instantiated_signature(candidate.function, &arguments, &[]);
            if candidate.params.len() == 1
                && self.types_equal(candidate.params[0].ty, signature.parameters[0])
            {
                return None;
            }
        }
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
                name: format!("{}.encode", owner.describe_name(self)),
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty: self.unit,
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
            crate::SourceFunctionDeclaration {
                name: "encode".into(),
            },
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
                    name: syntax::ident("encoder", span),
                    ty: signature.parameters[0],
                    calling: crate::FnParamCalling::Required,
                }],
                return_ty: self.unit,
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
        self.derived_encoding_methods
            .push((function, owner, encodable_ty));
        Some(function)
    }

    fn core_encoding_nominal(&self, name: &str) -> Option<hir::SourceNominalId> {
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
