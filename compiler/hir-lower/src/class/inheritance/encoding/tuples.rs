//! Complete ordinary method templates for tuple shapes used by this graph.

use super::*;
use la_arena::Arena;

impl Lowerer {
    pub(crate) fn prepare_tuple_encoding_templates(&mut self) {
        if !self.diagnostics.is_empty() {
            return;
        }
        let mut arities = self
            .types
            .values()
            .filter_map(|ty| match ty {
                Type::Tuple(elements) if !elements.is_empty() => Some(elements.len()),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        for template in self.tuple_encoding_templates.values() {
            arities.remove(&self.functions[template.function].type_param_count());
        }
        if arities.is_empty() {
            return;
        }
        let Some(encodable) = self.core_coding_nominal("Encodable") else {
            return;
        };
        let Some(encoder) = self.core_coding_nominal("Encoder") else {
            return;
        };
        let encodable = self.intern_interface_application(encodable, Vec::new());
        let encoder = self.intern_interface_application(encoder, Vec::new());
        let encodable = self.interface_applications[encodable].canonical_type;
        let encoder = self.interface_applications[encoder].canonical_type;
        let members = self.conformance_members(encodable);
        let [member] = members.as_slice() else { return };
        if member.signature.name != "encode"
            || member.signature.parameters != [encoder]
            || member.signature.result != self.unit
        {
            return;
        }
        let member = member.member;
        for arity in arities {
            self.declare_tuple_encoding(arity, encodable, encoder, member);
        }
    }

    fn declare_tuple_encoding(
        &mut self,
        arity: usize,
        encodable: TypeId,
        encoder: TypeId,
        member: hir::InterfaceMethodReference,
    ) {
        let span = Span::new(0, 0);
        let parameters = (0..arity)
            .map(|index| hir::TypeParamDecl {
                id: self.fresh_type_param(index),
                name: format!("T{index}"),
                bounds: hir::TypeParamBounds::Nominal(hir::NominalBounds {
                    class: None,
                    interfaces: vec![hir::InterfaceUpperBound {
                        ty: encodable,
                        span,
                    }],
                }),
                span,
            })
            .collect::<Vec<_>>();
        let elements = parameters
            .iter()
            .map(|parameter| self.intern_type(Type::Param(parameter.id)))
            .collect();
        let owner = self.intern_type(Type::Tuple(elements));
        let function = self.functions.alloc(hir::Function {
            signature: hir::CallableSignature {
                context_parameters: Vec::new(),
                release_callability: Default::default(),
                name: format!("$tuple.{arity}.encode"),
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty: self.unit,
                attributes: hir::FunctionAttributes::default(),
                span,
            },
            access: hir::DeclarationAccess {
                declared: hir::DeclaredVisibility::Public,
                lookup: hir::EffectiveLookupDomain(self.type_access_domain(owner)),
                slot: None,
            },
            genericity: hir::FunctionGenericity::Plain,
            kind: FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner,
                modifier: hir::MethodModifier::Final,
                dispatch: hir::MethodDispatch::Direct,
            }),
        });
        self.register_method_parameters(function, parameters.clone(), Vec::new());
        self.function_files.insert(function, self.current_file);
        self.signatures.insert(
            function,
            crate::FnSig {
                context_parameters: Vec::new(),
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                attributes: hir::FunctionAttributes::default(),
                owner_type_param_count: arity,
                type_params: parameters,
                params: vec![crate::FnParam {
                    name: syntax::ident("encoder", span),
                    calling: crate::FnParamCalling::Required,
                    ty: encoder,
                }],
                return_ty: self.unit,
            },
        );
        self.tuple_encoding_templates
            .alloc(hir::TupleEncodingTemplate {
                function,
                owner,
                interface: encodable,
                member,
            });
        let values = (0..arity)
            .map(|index| {
                (
                    ast::Expr::FieldAccess(ast::FieldAccess {
                        receiver: Box::new(ast::Expr::This { span }),
                        selector: ast::FieldSelector::Index(
                            u32::try_from(index + 1).expect("tuple indices fit u32"),
                            span,
                        ),
                        navigation: ast::Navigation::Direct,
                        span,
                    }),
                    span,
                )
            })
            .collect();
        let block = syntax::sequence(values, syntax::variable("encoder", span), span);
        let mut body = self.lower_synthesized_body(function, |lowerer| lowerer.lower_block(&block));
        for (id, local) in body.locals.iter_mut() {
            local.definition = hir::LocalValueDefinitionSite::Synthetic;
            if !matches!(
                local.selector,
                scoop_identity::LocalValueSelector::This
                    | scoop_identity::LocalValueSelector::Parameter { .. }
            ) {
                local.selector = scoop_identity::LocalValueSelector::Synthetic {
                    path: scoop_identity::StructuralDefinitionPath::from_first(
                        scoop_identity::StructuralPathSegment::new(
                            scoop_identity::StructuralDefinitionSiteRole::SyntheticValue,
                            id.into_raw().into_u32(),
                        ),
                        [],
                    ),
                    role: scoop_identity::SyntheticLocalRole::Temporary,
                };
            }
        }
        self.functions[function].kind = FunctionKind::User(body);
    }
}
