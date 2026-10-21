//! Shared declaration allocation for derived codecs and interface forwarders.

use super::*;

impl Lowerer {
    pub(in crate::class) fn allocate_synthesized_method(
        &mut self,
        owner: Owner,
        name: &str,
        mut signature: FnSig,
        modifier: hir::MethodModifier,
        visibility: ast::DeclaredVisibility,
        span: ast::Span,
    ) -> FunctionId {
        let host = self.owner_ty(owner);
        let parameters = self.owner_type_params(owner);
        signature.owner_type_param_count = parameters.len();
        signature.type_params = parameters.clone();
        let access = self.member_access(
            ast::VisibilitySyntax::Explicit { visibility, span },
            span,
            "method",
            owner,
            self.current_file,
            crate::visibility::MemberSlotAccess::Override,
        );
        let function = self.functions.alloc(hir::Function {
            signature: hir::CallableSignature {
                context_parameters: signature.context_parameters.clone(),
                release_callability: Default::default(),
                name: format!("{}.{}", owner.describe_name(self), name),
                is_suspend: signature.is_suspend,
                modifiers: signature.modifiers,
                params: Vec::new(),
                return_ty: signature.return_ty,
                attributes: signature.attributes,
                span,
            },
            access,
            genericity: hir::FunctionGenericity::Plain,
            kind: hir::FunctionKind::User(hir::Body {
                locals: la_arena::Arena::new(),
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner: host,
                modifier,
                dispatch: hir::MethodDispatch::Direct,
            }),
        });
        self.register_method_parameters(function, parameters, Vec::new());
        self.function_owner.insert(function, owner);
        self.record_function_file(function, self.current_file);
        self.signatures.insert(function, signature);
        match owner {
            Owner::Struct(id) => self.structs[id].methods.push(function),
            Owner::Enum(id) => self.enums[id].methods.push(function),
            Owner::Class(id) => self.classes[id].methods.push(function),
            Owner::Object(id) => self.classes[self.objects[id].backing_class]
                .methods
                .push(function),
            Owner::Interface(_) => unreachable!("interfaces declare their own implementations"),
        }
        function
    }
}
