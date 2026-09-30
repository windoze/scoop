//! Callable declaration allocation and overload-signature validation.

use super::*;

impl Lowerer {
    /// Allocate a member in its per-owner overload namespace. Signature and
    /// body population happen in later declaration passes.
    pub(crate) fn declare_method<'a>(
        &mut self,
        decl: &'a ast::FunctionDecl,
        owner: Owner,
        pending: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        let checked = self.check_function_annotations(decl, FunctionTarget::Member(owner));
        let host_ty = self.owner_ty(owner);
        let private_interface_member = matches!(owner, Owner::Interface(_))
            && matches!(
                decl.visibility,
                ast::VisibilitySyntax::Explicit {
                    visibility: ast::DeclaredVisibility::Private,
                    ..
                }
            );
        let modifier = match owner {
            Owner::Interface(_) if private_interface_member => hir::MethodModifier::Final,
            Owner::Interface(_) if matches!(decl.body, ast::FunctionBody::None) => {
                hir::MethodModifier::Abstract
            }
            Owner::Interface(_) => hir::MethodModifier::Open,
            Owner::Struct(_) | Owner::Enum(_) => hir::MethodModifier::Final,
            Owner::Object(_) => hir::MethodModifier::Final,
            Owner::Class(class_id)
                if self.classes[class_id].modifier == hir::ClassModifier::Final
                    && decl.is_override
                    && decl.modifier == ast::MethodModifier::Open =>
            {
                // Overrides are open by default, but a final owner closes the
                // dispatch family at this declaration.
                hir::MethodModifier::Final
            }
            Owner::Class(_) => match decl.modifier {
                ast::MethodModifier::Final => hir::MethodModifier::Final,
                ast::MethodModifier::Open => hir::MethodModifier::Open,
                ast::MethodModifier::Abstract => hir::MethodModifier::Abstract,
            },
        };
        let kind = match checked.intrinsic {
            Some(intrinsic) => FunctionKind::Intrinsic(intrinsic),
            None if modifier == hir::MethodModifier::Abstract => FunctionKind::Abstract {
                locals: Arena::new(),
            },
            None => FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
        };
        let name = format!("{}.{}", owner.describe_name(self), decl.name.text);
        let slot_access = if decl.is_override {
            crate::visibility::MemberSlotAccess::Override
        } else if modifier != hir::MethodModifier::Final {
            crate::visibility::MemberSlotAccess::Declared
        } else {
            crate::visibility::MemberSlotAccess::None
        };
        let access = self.member_access(
            decl.visibility,
            decl.name.span,
            "method",
            owner,
            file_index,
            slot_access,
        );
        let id = self.functions.alloc(Function {
            signature: hir::CallableSignature {
                name,
                is_suspend: decl.is_suspend,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty: self.unit,
                attributes: checked.attributes,
                span: decl.span,
            },

            access,
            genericity: hir::FunctionGenericity::Plain,
            kind,
            method: Some(hir::Method {
                owner: host_ty,
                modifier,
                dispatch: hir::MethodDispatch::Direct,
            }),
        });
        self.source_function_declarations.insert(
            id,
            crate::SourceFunctionDeclaration {
                name: decl.name.text.clone(),
            },
        );
        if let Some(intrinsic) = checked.intrinsic {
            self.register_intrinsic_function(id, intrinsic, decl.span);
        }
        self.function_owner.insert(id, owner);
        self.function_files.insert(id, file_index);
        match owner {
            Owner::Class(owner) => self.classes[owner].methods.push(id),
            Owner::Interface(owner) => {
                self.interface_methods
                    .get_mut(&owner)
                    .expect("the interface owner map was initialized above")
                    .push(id);
                if !private_interface_member {
                    let implementation = if matches!(decl.body, ast::FunctionBody::None) {
                        hir::InterfaceMemberImplementation::AbstractSlot
                    } else {
                        hir::InterfaceMemberImplementation::Body
                    };
                    let member = self.interface_method_entities.alloc(hir::InterfaceMethod {
                        owner,
                        function: id,
                        role: hir::InterfaceMemberRole::Function,
                        implementation,
                        overrides: Vec::new(),
                    });
                    self.functions[id]
                        .method
                        .as_mut()
                        .expect("declared interface function is a method")
                        .dispatch = hir::MethodDispatch::Interface(member);
                    self.interfaces[owner].methods.push(member);
                } else {
                    self.interfaces[owner].private_methods.push(id);
                }
            }
            Owner::Struct(owner) => self.structs[owner].methods.push(id),
            Owner::Enum(owner) => self.enums[owner].methods.push(id),
            Owner::Object(owner) => {
                let backing = self.objects[owner].backing_class;
                self.classes[backing].methods.push(id);
            }
        }
        pending.push((id, decl, file_index, owner));
    }

    /// Allocate a top-level declaration in either the ordinary or extension
    /// overload namespace; signature completeness is established later.
    pub(crate) fn declare_function<'a>(
        &mut self,
        decl: &'a ast::FunctionDecl,
        pending: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize)>,
        file_index: usize,
    ) {
        let checked = self.check_function_annotations(decl, FunctionTarget::TopLevel);
        let kind = match (checked.intrinsic, checked.extern_) {
            (Some(intrinsic), _) => FunctionKind::Intrinsic(intrinsic),
            (None, Some(extern_)) => {
                let id = self.extern_functions.alloc(hir::ExternFunction {
                    source_name: decl.name.text.clone(),
                    native_symbol: extern_.native_symbol,
                    library: extern_.library,
                    abi: extern_.abi,
                    calling_convention: checked.attributes.calling_convention,
                    gc_effect: checked.attributes.gc_effect,
                    safety: checked.attributes.safety,
                    params: Vec::new(),
                    return_type: self.unit,
                });
                FunctionKind::Extern(id)
            }
            (None, None) => FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
        };
        let access = self.top_level_access(decl.visibility, decl.name.span, "function", file_index);
        let id = self.functions.alloc(Function {
            signature: hir::CallableSignature {
                name: decl.name.text.clone(),
                is_suspend: decl.is_suspend,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty: self.unit,
                attributes: checked.attributes,
                span: decl.span,
            },

            access,
            genericity: hir::FunctionGenericity::Plain,
            kind,
            method: None,
        });
        self.source_function_declarations.insert(
            id,
            crate::SourceFunctionDeclaration {
                name: decl.name.text.clone(),
            },
        );
        if let Some(intrinsic) = checked.intrinsic {
            self.register_intrinsic_function(id, intrinsic, decl.span);
        }
        self.top_level.push(id);
        self.top_level_namespaces.register_function(
            file_index,
            decl.name.text.clone(),
            id,
            decl.receiver_ty.is_some(),
        );
        self.function_files.insert(id, file_index);
        pending.push((id, decl, file_index));
    }

    /// Reject declarations that differ only in return type after signatures
    /// have been fully resolved.
    pub(crate) fn validate_and_freeze_duplicate_signatures(
        &mut self,
        pending_functions: &[(FunctionId, &ast::FunctionDecl, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        let mut rejected = std::collections::HashSet::new();
        let mut ordered_functions = pending_functions.to_vec();
        ordered_functions.sort_by_key(|(id, _, _)| self.callable_declaration_order(*id));
        for (index, &(id, decl, file_index)) in ordered_functions.iter().enumerate() {
            let duplicates = ordered_functions[..index]
                .iter()
                .filter_map(|&(other, _, other_file)| {
                    (self
                        .top_level_namespaces
                        .sources_share_namespace(file_index, other_file)
                        && self.functions[other].name == decl.name.text
                        && self.same_parameter_signature(id, other)
                        && (self.functions[id].access.declared != hir::DeclaredVisibility::Private
                            || self.functions[other].access.declared
                                != hir::DeclaredVisibility::Private
                            || file_index == other_file))
                        .then_some(other)
                })
                .collect::<Vec<_>>();
            if !duplicates.is_empty() {
                rejected.insert(id);
                rejected.extend(duplicates);
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
        let mut ordered_methods = pending_methods.to_vec();
        ordered_methods.sort_by_key(|(id, _, _, _)| self.callable_declaration_order(*id));
        for (index, &(id, decl, file_index, owner)) in ordered_methods.iter().enumerate() {
            let duplicates = ordered_methods[..index]
                .iter()
                .filter_map(|&(other, _, _, other_owner)| {
                    (other_owner == owner
                        && self.functions[other].name == self.functions[id].name
                        && self.same_parameter_signature(id, other))
                    .then_some(other)
                })
                .collect::<Vec<_>>();
            if !duplicates.is_empty() {
                rejected.insert(id);
                rejected.extend(duplicates);
                let host = owner.describe(self);
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` in {host} is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
        self.declaration_surface.freeze(rejected);
    }

    /// Persistent source identity, followed by source span, defines declaration
    /// order independently from dense container positions and display paths.
    fn callable_declaration_order(
        &self,
        function: FunctionId,
    ) -> (scoop_identity::SourceIdentity, u32, u32, u32) {
        let file = self.function_files[&function];
        let source = self.visibility_file(file);
        let span = self.functions[function].span;
        (source, span.start, span.end, function.into_raw().into_u32())
    }

    /// Compare parameter signatures under the exact alpha-renaming relation
    /// between their globally unique type-parameter identities.
    pub(crate) fn same_parameter_signature(&self, a: FunctionId, b: FunctionId) -> bool {
        let (Some(a_sig), Some(b_sig)) = (self.signatures.get(&a), self.signatures.get(&b)) else {
            return false;
        };
        let a_sig = a_sig.clone();
        let b_sig = b_sig.clone();
        if a_sig.type_params.len() != b_sig.type_params.len() {
            return false;
        }
        let parameter_pairs = a_sig
            .type_params
            .iter()
            .zip(&b_sig.type_params)
            .map(|(a, b)| (a.id, b.id))
            .collect::<Vec<_>>();
        let receivers_match = match (
            self.extension_receivers.get(&a),
            self.extension_receivers.get(&b),
        ) {
            (Some(&a), Some(&b)) => self.signature_types_equal(a, b, &parameter_pairs),
            (None, None) => true,
            _ => false,
        };
        receivers_match
            && a_sig.params.len() == b_sig.params.len()
            && a_sig
                .params
                .iter()
                .zip(&b_sig.params)
                .all(|(x, y)| self.signature_types_equal(x.ty, y.ty, &parameter_pairs))
    }

    fn signature_types_equal(
        &self,
        left: TypeId,
        right: TypeId,
        parameter_pairs: &[(hir::TypeParamId, hir::TypeParamId)],
    ) -> bool {
        match (&self.types[left], &self.types[right]) {
            (Type::Param(left), Type::Param(right)) => {
                parameter_pairs
                    .iter()
                    .any(|&(expected_left, expected_right)| {
                        expected_left == *left && expected_right == *right
                    })
            }
            (Type::Struct(left), Type::Struct(right)) => {
                let left = &self.struct_applications[*left];
                let right = &self.struct_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Class(left), Type::Class(right)) => {
                let left = &self.class_applications[*left];
                let right = &self.class_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Interface(left), Type::Interface(right)) => {
                let left = &self.interface_applications[*left];
                let right = &self.interface_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Enum(left), Type::Enum(right)) => {
                let left = &self.enum_applications[*left];
                let right = &self.enum_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Tuple(left), Type::Tuple(right)) => {
                self.signature_type_lists_equal(left, right, parameter_pairs)
            }
            (Type::Function(left), Type::Function(right))
            | (Type::FunPtr(left), Type::FunPtr(right)) => {
                let left = &self.function_types[*left];
                let right = &self.function_types[*right];
                left.is_suspend == right.is_suspend
                    && self.signature_type_lists_equal(
                        &left.parameter_types,
                        &right.parameter_types,
                        parameter_pairs,
                    )
                    && self.signature_types_equal(
                        left.return_type,
                        right.return_type,
                        parameter_pairs,
                    )
            }
            (Type::Ptr(left), Type::Ptr(right)) => {
                self.signature_types_equal(*left, *right, parameter_pairs)
            }
            _ => self.types_equal(left, right),
        }
    }

    fn signature_type_lists_equal(
        &self,
        left: &[TypeId],
        right: &[TypeId],
        parameter_pairs: &[(hir::TypeParamId, hir::TypeParamId)],
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(&left, &right)| self.signature_types_equal(left, right, parameter_pairs))
    }
}
