use super::*;
use crate::NominalTarget;
use crate::imports::lookup::calls::{ExpressionQualifierLookup, ExpressionQualifierTarget};

mod extensions;
mod interface_super;
mod pointers;
mod primitives;
mod resolution;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::expr) enum PropertyExtensionInvokeOrigin {
    Member(hir::PropertyId),
    NamedValue(crate::imports::lookup::values::ValueTarget),
    Extension(crate::imports::lookup::calls::ExtensionPropertyIdentity),
}

pub(in crate::expr) struct PropertyExtensionInvokeInput {
    origin: PropertyExtensionInvokeOrigin,
    property: SuccessfulExprLayer,
    candidates: Vec<hir::FunctionId>,
}

impl PropertyExtensionInvokeInput {
    pub(in crate::expr) fn new(
        origin: PropertyExtensionInvokeOrigin,
        property: SuccessfulExprLayer,
        candidates: Vec<hir::FunctionId>,
    ) -> Self {
        Self {
            origin,
            property,
            candidates,
        }
    }
}

pub(in crate::expr) enum PropertyExtensionInvokeOutcome {
    NoApplicable(Option<Box<Lowerer>>),
    Blocked,
    Failed(Box<Lowerer>),
    Resolved(SuccessfulExprLayer),
}

impl Lowerer {
    /// A direct receiver spelling is still an expression name first. Keep
    /// this guard shared by every qualifier consumer so none of them can
    /// bypass a higher lexical/member value while probing a lower type.
    fn lexical_or_member_value_blocks_type_qualifier(&self, name: &str) -> bool {
        let initializing_field = self.initialization_context.is_some() && {
            let mut state = self.clone();
            state.initializing_receiver_has_field(name)
        };
        let object_const = match self.current_owner {
            Some(crate::Owner::Object(object)) => self.classes[self.objects[object].backing_class]
                .properties
                .iter()
                .any(|property| {
                    self.properties[*property].name == name
                        && matches!(
                            self.properties[*property].representation,
                            hir::PropertyRepresentation::Const { .. }
                        )
                }),
            _ => false,
        };
        (name == "field" && self.backing_field_context.is_some())
            || self.scopes.lookup(name).is_some()
            || !self.local_function_scopes.lookup(name).is_empty()
            || self.available_capture(name).is_some()
            || self.constructor_params_in_scope.contains_key(name)
            || initializing_field
            || object_const
            || self.host_has_property(name)
    }

    pub(crate) fn nominal_qualifier_target(&self, expression: &ast::Expr) -> Option<NominalTarget> {
        match expression {
            ast::Expr::Var(name)
                if !self.lexical_or_member_value_blocks_type_qualifier(&name.text) =>
            {
                self.lexical_nested_nominal_target(&name.text).or_else(|| {
                    match self.lookup_expression_qualifier(name) {
                        ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Object(
                            object,
                        ))
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::Object(object),
                        ) => Some(NominalTarget::Object(object)),
                        ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                            crate::namespace::TopLevelTypeTarget::Nominal(target),
                        ))
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::Type(
                                crate::namespace::TopLevelTypeTarget::Nominal(target),
                            ),
                        ) => Some(target),
                        ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                            crate::namespace::TopLevelTypeTarget::Alias(alias),
                        ))
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::Type(
                                crate::namespace::TopLevelTypeTarget::Alias(alias),
                            ),
                        ) => self
                            .resolved_type_alias_target(alias)
                            .and_then(|target| self.nominal_target_for_type(target)),
                        ExpressionQualifierLookup::Missing
                        | ExpressionQualifierLookup::Value
                        | ExpressionQualifierLookup::Ambiguous => None,
                    }
                })
            }
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
                let ast::FieldSelector::Name(name) = &access.selector else {
                    return None;
                };
                let owner = self.nominal_qualifier_target(&access.receiver)?.owner();
                self.nested_nominal_target(owner, &name.text)
            }
            _ => None,
        }
    }

    /// Resolve a direct alias qualifier after value bindings have had their
    /// normal shadowing opportunity. This uses the resolver-owned API so the
    /// access diagnostic is identical in type and expression positions.
    pub(in crate::expr) fn resolve_direct_alias_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<(AliasExpansion, NominalTarget)>, ()> {
        let Some((target, nominal)) = self.resolve_direct_type_alias_qualifier(expression)? else {
            return Ok(None);
        };
        let ast::Expr::Var(name) = expression else {
            unreachable!("a direct alias qualifier is a source name")
        };
        Ok(Some((
            AliasExpansion {
                name: name.clone(),
                target,
            },
            nominal,
        )))
    }

    pub(crate) fn resolve_direct_type_alias_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<(TypeId, NominalTarget)>, ()> {
        let ast::Expr::Var(name) = expression else {
            return Ok(None);
        };
        if self.lexical_or_member_value_blocks_type_qualifier(&name.text)
            || self.lexical_nested_nominal_target(&name.text).is_some()
        {
            return Ok(None);
        }
        let alias = match self.lookup_expression_qualifier(name) {
            ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                crate::namespace::TopLevelTypeTarget::Alias(alias),
            ))
            | ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Type(
                crate::namespace::TopLevelTypeTarget::Alias(alias),
            )) => alias,
            _ => return Ok(None),
        };
        let Some(target) = self.resolve_type_alias_id_reference(alias, name, false) else {
            return Err(());
        };
        let Some(nominal) = self.nominal_target_for_type(target) else {
            self.error(
                name.span,
                format!("typealias `{}` does not name a type qualifier", name.text),
            );
            return Err(());
        };
        Ok(Some((target, nominal)))
    }

    fn lower_static_nested_constructor(
        &mut self,
        target: NominalTarget,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match target {
            NominalTarget::Struct(struct_id) => {
                let application = self.structs[struct_id].self_application;
                let ty = self.struct_applications[application].canonical_type;
                if !self.nominal_is_accessible(ty) {
                    self.error(
                        name.span,
                        format!("struct `{}` is not accessible here", name.text),
                    );
                    return None;
                }
                self.lower_struct_init(struct_id, ty, call, sink, expected)
            }
            NominalTarget::Class(class_id) => {
                let application = self.classes[class_id].self_application;
                let ty = self.class_applications[application].canonical_type;
                if !self.nominal_is_accessible(ty) {
                    self.error(
                        name.span,
                        format!("class `{}` is not accessible here", name.text),
                    );
                    return None;
                }
                self.lower_class_construct(class_id, call, sink, expected)
            }
            NominalTarget::Enum(_) => {
                self.error(
                    name.span,
                    format!(
                        "enum `{}` cannot be constructed without a variant",
                        name.text
                    ),
                );
                None
            }
            NominalTarget::Interface(_) => {
                self.error(
                    name.span,
                    format!("interface `{}` cannot be constructed", name.text),
                );
                None
            }
            NominalTarget::Object(object) => {
                let kind = match self.objects[object].kind {
                    hir::ObjectKind::Standalone => "object",
                    hir::ObjectKind::Companion(_) => "companion object",
                };
                self.error(
                    name.span,
                    format!("{kind} `{}` cannot be constructed", name.text),
                );
                None
            }
        }
    }

    /// Resolve `super.name(...)` from the exact direct-base application. No
    /// extension, property-like, or interface layer participates, and the
    /// resulting HIR variant preserves the mandatory direct-dispatch proof.
    pub(super) fn lower_super_method_call(
        &mut self,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if self.initialization_context.is_some() {
            self.error(
                call.span,
                "initializing receiver cannot escape before construction completes".into(),
            );
            return None;
        }
        let Some(mut receiver) = self.lower_current_this(call.span) else {
            self.error(
                call.span,
                "`super` method calls are only allowed inside class member functions".into(),
            );
            return None;
        };
        let Type::Class(application) = self.types[receiver.ty] else {
            self.error(
                call.span,
                "`super` method calls require a class receiver".into(),
            );
            return None;
        };
        let current = self.class_applications[application].clone();
        let Some(base) = self.classes[current.template].base_class else {
            self.error(
                call.span,
                format!(
                    "class `{}` has no direct base for `super.{}`",
                    self.classes[current.template].name, name.text
                ),
            );
            return None;
        };
        let base = self.instantiate_ty(base, &current.arguments);
        let Type::Class(base_application) = self.types[base] else {
            unreachable!("a class direct base is a class application")
        };
        receiver.ty = base;
        let candidates = self.methods_by_name(base, &name.text);
        if candidates.is_empty() {
            let base_name = self.classes[self.class_applications[base_application].template]
                .name
                .clone();
            self.error(
                name.span,
                format!("base class `{base_name}` has no method `{}`", name.text),
            );
            return None;
        }
        self.finish_super_method_call(candidates, &name.text, receiver, call, sink, expected)
    }

    /// `this` (M6): only inside member functions, where it is
    /// parameter 0 (`lower_body` registers it as a local).
    pub(super) fn lower_this(&mut self, span: Span) -> Option<hir::Expr> {
        if self.reject_initializing_this(span) {
            return None;
        }
        let Some(this) = self.lower_current_this(span) else {
            self.error(
                span,
                "`this` is only allowed inside member functions".to_string(),
            );
            return None;
        };
        Some(this)
    }

    /// `receiver.name(args)` (M6/M7): the method overloads are
    /// collected from the receiver's static type — class members (base
    /// chain included), interface methods, or struct / enum methods — and
    /// resolved by the unified M16 algorithm. If that layer has no applicable
    /// candidate, visible extensions are probed in import priority order.
    /// Single and multiple candidates use the same entry.
    /// The dispatch kind (direct / virtual / interface) is decided at
    /// MIR from the receiver's static type (hir docs).
    ///
    /// One receiver shape is not a method call: the M6 parser folds a
    /// qualified enum variant construction `E.V(args)` into this
    /// syntax (`MethodCall { receiver: Var("E"), ... }`). When the
    /// receiver is a bare name that is no in-scope variable and no
    /// property of the current host — but names an enum — it is a
    /// variant path and goes through variant construction (M4 rules:
    /// variant existence, per-field argument checks, type-argument
    /// inference, constructor-style defaults). Variables and host
    /// properties shadow enum names. Core array conversion methods enter the
    /// ordinary member candidate layer and are normalized only after their
    /// typed intrinsic target wins (spec 10.4).
    pub(super) fn lower_method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if matches!(receiver, ast::Expr::This { .. }) && self.initialization_context.is_some() {
            if !self.initializing_receiver_has_field(&name.text) {
                self.error(
                    call.span,
                    "initializing receiver cannot escape before construction completes".into(),
                );
                return None;
            }
            let property = self.initializing_field(name, call.span)?;
            return match self.probe_property_member_invoke_partition(
                property.read,
                call,
                expected,
                false,
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    Some(self.commit_expr_layer(layer, sink))
                }
                PropertyExtensionInvokeOutcome::Blocked => None,
                PropertyExtensionInvokeOutcome::Failed(failure)
                | PropertyExtensionInvokeOutcome::NoApplicable(Some(failure)) => {
                    self.commit_layer_diagnostics(*failure);
                    None
                }
                PropertyExtensionInvokeOutcome::NoApplicable(None) => {
                    self.error(
                        call.span,
                        "initializing receiver cannot escape before construction completes".into(),
                    );
                    None
                }
            };
        }
        let direct_alias = match self.resolve_direct_alias_qualifier(receiver) {
            Ok(alias) => alias,
            Err(()) => return None,
        };
        let qualifier = direct_alias
            .as_ref()
            .map(|(_, target)| *target)
            .or_else(|| self.nominal_qualifier_target(receiver));
        if let Some(qualifier) = qualifier {
            if let Some(target) = self.nested_nominal_target(qualifier.owner(), &name.text) {
                return self.lower_static_nested_constructor(target, name, call, sink, expected);
            }
            if let NominalTarget::Enum(enum_id) = qualifier {
                if let Some(target) = self.find_variant_ref(enum_id, &name.text) {
                    let expected = self.alias_fixed_expected(
                        direct_alias.as_ref().map(|(alias, _)| alias),
                        call.type_args,
                        expected,
                    )?;
                    return self.lower_variant_construct(target, call, sink, expected);
                }
            }
            let forwarded = self.companion_forwarding_object(qualifier, &name.text);
            if let NominalTarget::Object(object) = qualifier
                && forwarded.is_none()
            {
                let receiver = self.lower_singleton_value(object, receiver.span())?;
                return self.lower_explicit_named_call(
                    receiver,
                    name,
                    call,
                    sink,
                    expected,
                    RequiredCallableModifiers::default(),
                );
            }
            if let Some(companion) = forwarded {
                let receiver = self.lower_singleton_value(companion, receiver.span())?;
                return self.lower_explicit_named_call(
                    receiver,
                    name,
                    call,
                    sink,
                    expected,
                    RequiredCallableModifiers::default(),
                );
            }
            if let NominalTarget::Enum(_) = qualifier {
                self.error(
                    name.span,
                    format!(
                        "enum `{}` has no variant `{}`",
                        qualifier.owner().describe_name(self),
                        name.text
                    ),
                );
                return None;
            }
            self.error(
                name.span,
                format!(
                    "type `{}` has no nested type `{}`",
                    qualifier.owner().describe_name(self),
                    name.text
                ),
            );
            return None;
        }
        if let Some(layer) = self.probe_integer_literal_receiver(
            receiver,
            expected,
            |state, receiver, layer_sink| {
                state.lower_explicit_named_call(
                    receiver,
                    name,
                    call,
                    layer_sink,
                    expected,
                    RequiredCallableModifiers::default(),
                )
            },
        ) {
            return Some(self.commit_expr_layer(layer, sink));
        }
        let receiver = self.lower_expr(receiver, sink, None)?;
        self.lower_explicit_named_call(
            receiver,
            name,
            call,
            sink,
            expected,
            RequiredCallableModifiers::default(),
        )
    }

    fn lower_explicit_named_call(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        direct_required: RequiredCallableModifiers,
    ) -> Option<hir::Expr> {
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::Function(_)) {
            return self.lower_named_call_on_receiver(
                receiver,
                name,
                call,
                sink,
                expected,
                direct_required,
            );
        }
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::FunPtr(_)) {
            self.error(
                call.span,
                "FunPtr values are not callable in Scoop".to_string(),
            );
            return None;
        }
        let mut first_failure = None;
        let member_property = self
            .find_accessible_nominal_property(receiver.ty, &name.text)
            .map(|(property, _, _)| property);
        let property = match self
            .probe_expr_layer(|state, _| state.member_property_read(receiver.clone(), name))
        {
            Ok(layer) => Some(layer),
            Err(failure) if failure.diagnostics.len() > self.diagnostics.len() => {
                first_failure = Some(failure);
                None
            }
            Err(_) => None,
        };
        let mut members = self.methods_by_name(receiver.ty, &name.text);
        members.retain(|candidate| {
            Self::matches_required_modifiers(
                self.signatures[&candidate.function].modifiers,
                direct_required,
            )
        });
        if !members.is_empty() && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = members.len();
            members.retain(|candidate| {
                let signature = &self.signatures[&candidate.function];
                signature.type_params.len() == signature.owner_type_param_count
            });
            if members.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                let mut failure = self.clone();
                failure.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be called through interface type `{found}`",
                        name.text
                    ),
                );
                first_failure = Some(Box::new(failure));
            }
        }
        if !members.is_empty() {
            match self.probe_member_call_partition(
                members,
                &name.text,
                receiver.clone(),
                call,
                expected,
                false,
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    first_failure = failure;
                }
            }
        }

        if let Some(property) = &property {
            match property.state.probe_property_member_invoke_partition(
                property.expression.clone(),
                call,
                expected,
                direct_required.infix,
            ) {
                PropertyExtensionInvokeOutcome::Resolved(mut layer) => {
                    let mut setup = property.sink.clone();
                    setup.append(&mut layer.sink);
                    layer.sink = setup;
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    if let Some(failure) = failure {
                        first_failure.get_or_insert(failure);
                    }
                }
            }
        }

        let mut extension_layers = match direct_required.operator {
            Some(operator) => self.named_extension_operator_layers(operator),
            None => self.named_extension_call_layers(&name.text),
        };
        let mut invoke_layers = self.named_extension_call_layers("invoke");
        let mut extension_property_layers = self.named_extension_property_layers(&name.text);
        Self::retain_highest_rank_origins(&mut extension_layers);
        Self::retain_highest_rank_origins(&mut invoke_layers);
        Self::retain_highest_rank_origins(&mut extension_property_layers);
        for rank in 0..=3 {
            let suppressed_extensions =
                Self::suppressed_extensions_at_rank(&extension_layers, rank, |function| {
                    Self::matches_required_modifiers(
                        self.signatures[&function].modifiers,
                        direct_required,
                    )
                });
            let suppressed_invokes =
                Self::suppressed_extensions_at_rank(&invoke_layers, rank, |function| {
                    Self::matches_required_modifiers(
                        self.signatures[&function].modifiers,
                        RequiredCallableModifiers {
                            operator: Some(hir::OperatorKind::Invoke),
                            infix: direct_required.infix,
                            ..Default::default()
                        },
                    )
                });
            let extensions =
                Self::extension_candidates_at_rank(&extension_layers, rank, |function| {
                    Self::matches_required_modifiers(
                        self.signatures[&function].modifiers,
                        direct_required,
                    )
                });
            if !extensions.is_empty() {
                match self.probe_extension_call_partition(
                    &extensions,
                    &name.text,
                    receiver.clone(),
                    call,
                    expected,
                    false,
                ) {
                    PropertyExtensionInvokeOutcome::Resolved(layer) => {
                        return Some(self.commit_expr_layer(layer, sink));
                    }
                    PropertyExtensionInvokeOutcome::Blocked => return None,
                    PropertyExtensionInvokeOutcome::Failed(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                    PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                        if let Some(failure) = failure {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }

            let invokes = Self::extension_candidates_at_rank(&invoke_layers, rank, |function| {
                Self::matches_required_modifiers(
                    self.signatures[&function].modifiers,
                    RequiredCallableModifiers {
                        operator: Some(hir::OperatorKind::Invoke),
                        infix: direct_required.infix,
                        ..Default::default()
                    },
                )
            });
            if let Some(property) = &property
                && !invokes.is_empty()
            {
                match self.probe_property_extension_invoke_partition(
                    vec![PropertyExtensionInvokeInput::new(
                        PropertyExtensionInvokeOrigin::Member(
                            member_property.expect("a successful member property read has an id"),
                        ),
                        property.clone(),
                        invokes,
                    )],
                    call,
                    expected,
                    Self::extension_layer_name(rank),
                ) {
                    PropertyExtensionInvokeOutcome::Resolved(layer) => {
                        return Some(self.commit_expr_layer(layer, sink));
                    }
                    PropertyExtensionInvokeOutcome::Blocked => return None,
                    PropertyExtensionInvokeOutcome::Failed(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                    PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                        if let Some(failure) = failure {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }

            let property_candidates =
                Self::property_candidates_at_rank(&extension_property_layers, rank);
            if !property_candidates.is_empty() {
                let mut extension_property_state = self.clone();
                let mut extension_property_sink = Vec::new();
                match extension_property_state.resolve_extension_property_candidates_outcome(
                    receiver.clone(),
                    name,
                    &property_candidates,
                    &mut extension_property_sink,
                ) {
                    crate::properties::ExtensionPropertyCandidateOutcome::Resolved(property) => {
                        match extension_property_state.probe_property_member_invoke_partition(
                            property.read,
                            call,
                            expected,
                            direct_required.infix,
                        ) {
                            PropertyExtensionInvokeOutcome::Resolved(mut layer) => {
                                extension_property_sink.append(&mut layer.sink);
                                layer.sink = extension_property_sink;
                                return Some(self.commit_expr_layer(layer, sink));
                            }
                            PropertyExtensionInvokeOutcome::Blocked => return None,
                            PropertyExtensionInvokeOutcome::Failed(failure) => {
                                self.commit_layer_diagnostics(*failure);
                                return None;
                            }
                            PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                                if let Some(failure) = failure {
                                    first_failure.get_or_insert(failure);
                                }
                            }
                        }
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::Failed => {
                        self.commit_layer_diagnostics(extension_property_state);
                        return None;
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::NoApplicable => {
                        if extension_property_state.diagnostics.len() > self.diagnostics.len() {
                            first_failure.get_or_insert(Box::new(extension_property_state));
                        }
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::NoCandidate => {}
                }
            }

            let mut property_extension_inputs = Vec::new();
            for property_layer in &extension_property_layers {
                for invoke_layer in &invoke_layers {
                    if property_layer
                        .kind
                        .call_rank()
                        .max(invoke_layer.kind.call_rank())
                        != rank
                    {
                        continue;
                    }
                    let invokes = invoke_layer
                        .candidates
                        .iter()
                        .copied()
                        .filter(|function| {
                            Self::matches_required_modifiers(
                                self.signatures[function].modifiers,
                                RequiredCallableModifiers {
                                    operator: Some(hir::OperatorKind::Invoke),
                                    infix: direct_required.infix,
                                    ..Default::default()
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    if property_layer.candidates.is_empty() || invokes.is_empty() {
                        continue;
                    }
                    let mut property_state = self.clone();
                    let mut property_sink = Vec::new();
                    match property_state.resolve_extension_property_candidates_outcome(
                        receiver.clone(),
                        name,
                        &property_layer.candidates,
                        &mut property_sink,
                    ) {
                        crate::properties::ExtensionPropertyCandidateOutcome::Resolved(
                            property,
                        ) => {
                            property_extension_inputs.push(PropertyExtensionInvokeInput::new(
                                PropertyExtensionInvokeOrigin::Extension(property.identity()),
                                SuccessfulExprLayer {
                                    state: Box::new(property_state),
                                    expression: property.read,
                                    sink: property_sink,
                                },
                                invokes,
                            ));
                        }
                        crate::properties::ExtensionPropertyCandidateOutcome::Failed => {
                            self.commit_layer_diagnostics(property_state);
                            return None;
                        }
                        crate::properties::ExtensionPropertyCandidateOutcome::NoApplicable => {
                            if property_state.diagnostics.len() > self.diagnostics.len() {
                                first_failure.get_or_insert(Box::new(property_state));
                            }
                        }
                        crate::properties::ExtensionPropertyCandidateOutcome::NoCandidate => {}
                    }
                }
            }
            if !property_extension_inputs.is_empty() {
                match self.probe_property_extension_invoke_partition(
                    property_extension_inputs,
                    call,
                    expected,
                    Self::extension_layer_name(rank),
                ) {
                    PropertyExtensionInvokeOutcome::Resolved(layer) => {
                        return Some(self.commit_expr_layer(layer, sink));
                    }
                    PropertyExtensionInvokeOutcome::Blocked => return None,
                    PropertyExtensionInvokeOutcome::Failed(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                    PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                        if let Some(failure) = failure {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }
            if suppressed_extensions || suppressed_invokes {
                if let Some(failure) = first_failure {
                    self.commit_layer_diagnostics(*failure);
                }
                return None;
            }
        }

        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
        } else if let Some(message) = self.inaccessible_method_message(receiver.ty, &name.text) {
            self.error(name.span, message);
        } else {
            let found = self.type_name(receiver.ty);
            let capability = if direct_required.infix {
                "infix callable"
            } else {
                "method"
            };
            self.error(
                name.span,
                format!("type `{found}` has no {capability} `{}`", name.text),
            );
        }
        None
    }

    fn matches_required_modifiers(
        modifiers: hir::CallableModifiers,
        required: RequiredCallableModifiers,
    ) -> bool {
        required
            .operator
            .is_none_or(|operator| modifiers.operator == Some(operator))
            && required
                .property_delegate_operator
                .is_none_or(|operator| modifiers.property_delegate_operator == Some(operator))
            && (!required.infix || modifiers.is_infix)
    }

    fn extension_candidates_at_rank(
        layers: &[crate::imports::lookup::LookupLayer<hir::FunctionId>],
        rank: usize,
        predicate: impl Fn(hir::FunctionId) -> bool,
    ) -> Vec<hir::FunctionId> {
        layers
            .iter()
            .filter(|layer| layer.kind.call_rank() == rank)
            .flat_map(|layer| layer.candidates.iter().copied())
            .filter(|function| predicate(*function))
            .collect()
    }

    fn suppressed_extensions_at_rank(
        layers: &[crate::imports::lookup::LookupLayer<hir::FunctionId>],
        rank: usize,
        predicate: impl Fn(hir::FunctionId) -> bool,
    ) -> bool {
        layers
            .iter()
            .filter(|layer| layer.kind.call_rank() == rank)
            .flat_map(|layer| layer.suppressed_callables.iter().copied())
            .any(predicate)
    }

    fn property_candidates_at_rank(
        layers: &[crate::imports::lookup::LookupLayer<
            crate::imports::lookup::calls::ExtensionPropertyTarget,
        >],
        rank: usize,
    ) -> Vec<crate::imports::lookup::calls::ExtensionPropertyTarget> {
        layers
            .iter()
            .filter(|layer| layer.kind.call_rank() == rank)
            .flat_map(|layer| layer.candidates.iter().cloned())
            .collect()
    }

    fn retain_highest_rank_origins<T: Clone + PartialEq>(
        layers: &mut [crate::imports::lookup::LookupLayer<T>],
    ) {
        let mut seen = Vec::new();
        for layer in layers {
            layer.candidates.retain(|candidate| {
                if seen.contains(candidate) {
                    false
                } else {
                    seen.push(candidate.clone());
                    true
                }
            });
        }
    }

    fn extension_layer_name(rank: usize) -> &'static str {
        match rank {
            0 => "exact import",
            1 => "current package",
            2 => "star import",
            3 => "core prelude",
            _ => unreachable!("extension layer ranks are a closed four-layer set"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn probe_member_call_partition(
        &self,
        candidates: Vec<crate::CallableCandidate>,
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        operator_set: bool,
    ) -> PropertyExtensionInvokeOutcome {
        use crate::overload::{CallArgumentProtocol, OverloadCall, OverloadResolutionOutcome};

        if candidates.is_empty() {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        }
        let mut state = self.clone();
        let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args) else {
            return PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state)));
        };
        let mut layer_sink = Vec::new();
        match state.resolve_member_overload_outcome(
            name,
            &candidates,
            receiver,
            OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
                expected_result: expected,
                argument_protocol: if operator_set {
                    CallArgumentProtocol::OperatorSet
                } else {
                    CallArgumentProtocol::Ordinary
                },
            },
            &mut layer_sink,
        ) {
            OverloadResolutionOutcome::NoApplicable => {
                PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state)))
            }
            OverloadResolutionOutcome::Blocked => PropertyExtensionInvokeOutcome::Blocked,
            OverloadResolutionOutcome::Failed => {
                PropertyExtensionInvokeOutcome::Failed(Box::new(state))
            }
            OverloadResolutionOutcome::Resolved(resolved) => {
                let expression = state
                    .finish_resolved_method_call(*resolved, call.span)
                    .expect("a resolved member call always materializes an expression");
                PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                    state: Box::new(state),
                    expression,
                    sink: layer_sink,
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn probe_extension_call_partition(
        &self,
        candidates: &[hir::FunctionId],
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        operator_set: bool,
    ) -> PropertyExtensionInvokeOutcome {
        use crate::overload::{CallArgumentProtocol, OverloadCall, OverloadResolutionOutcome};

        if candidates.is_empty() {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        }
        let mut state = self.clone();
        let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args) else {
            return PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state)));
        };
        let mut layer_sink = Vec::new();
        match state.resolve_extension_overload_outcome(
            name,
            candidates,
            receiver,
            OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
                expected_result: expected,
                argument_protocol: if operator_set {
                    CallArgumentProtocol::OperatorSet
                } else {
                    CallArgumentProtocol::Ordinary
                },
            },
            &mut layer_sink,
        ) {
            OverloadResolutionOutcome::NoApplicable => {
                PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state)))
            }
            OverloadResolutionOutcome::Blocked => PropertyExtensionInvokeOutcome::Blocked,
            OverloadResolutionOutcome::Failed => {
                PropertyExtensionInvokeOutcome::Failed(Box::new(state))
            }
            OverloadResolutionOutcome::Resolved(resolved) => {
                let resolved = *resolved;
                let callee = state.materialize_resolved_callee(&resolved);
                state.check_call_effects(callee, call.span);
                let expression = hir::Expr {
                    kind: hir::ExprKind::Call {
                        callee,
                        args: resolved.args,
                    },
                    ty: resolved.return_ty,
                    span: call.span,
                    origin: state.expression_origin(call.span),
                };
                PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                    state: Box::new(state),
                    expression,
                    sink: layer_sink,
                })
            }
        }
    }

    pub(in crate::expr) fn member_property_read(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
    ) -> Option<hir::Expr> {
        let receiver_ty = receiver.ty;
        if let Some((property, owner, ty)) =
            self.find_accessible_nominal_property(receiver_ty, &name.text)
        {
            return self.lower_property_read(property, Some(owner), Some(receiver), ty, name.span);
        }
        None
    }

    pub(in crate::expr) fn probe_property_member_invoke_partition(
        &self,
        property: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        require_infix: bool,
    ) -> PropertyExtensionInvokeOutcome {
        if matches!(self.types[property.ty], Type::Function(_)) {
            if require_infix {
                return PropertyExtensionInvokeOutcome::NoApplicable(None);
            }
            return match self.probe_expr_layer(|state, layer_sink| {
                state.lower_named_call_on_receiver(
                    property,
                    &ast::Ident {
                        text: "invoke".to_string(),
                        span: call.span,
                    },
                    call,
                    layer_sink,
                    expected,
                    RequiredCallableModifiers::default(),
                )
            }) {
                Ok(layer) => PropertyExtensionInvokeOutcome::Resolved(layer),
                Err(failure) => PropertyExtensionInvokeOutcome::NoApplicable(Some(failure)),
            };
        }
        let mut candidate_state = self.clone();
        let mut candidates = candidate_state.methods_by_name(property.ty, "invoke");
        candidates.retain(|candidate| {
            Self::matches_required_modifiers(
                candidate_state.signatures[&candidate.function].modifiers,
                RequiredCallableModifiers {
                    operator: Some(hir::OperatorKind::Invoke),
                    infix: require_infix,
                    ..Default::default()
                },
            )
        });
        candidate_state
            .probe_member_call_partition(candidates, "invoke", property, call, expected, false)
    }

    pub(in crate::expr) fn probe_property_extension_invoke_partition(
        &self,
        inputs: Vec<PropertyExtensionInvokeInput>,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        layer_name: &str,
    ) -> PropertyExtensionInvokeOutcome {
        use crate::call_resolution::named::NamedFunctionLikeProbe;
        use crate::overload::{CallArgumentProtocol, NamedCallReceiver, OverloadCall};

        let mut probes = Vec::new();
        let mut setups = Vec::new();
        let mut first_failure = None;
        let mut seen = Vec::new();
        let mut suppressed = false;
        for input in inputs {
            for function in input.candidates {
                if self.declaration_surface.rejects_function(function) {
                    suppressed = true;
                    continue;
                }
                if seen.contains(&(input.origin, function)) {
                    continue;
                }
                seen.push((input.origin, function));
                let mut state = (*input.property.state).clone();
                let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args) else {
                    first_failure.get_or_insert(Box::new(state));
                    continue;
                };
                let target = crate::CallableCandidate::function(
                    function,
                    Vec::new(),
                    state.function_lookup_witness(function),
                );
                match state.probe_named_callable(
                    "invoke",
                    target,
                    NamedCallReceiver::Extension(input.property.expression.clone()),
                    OverloadCall {
                        explicit_type_args: &explicit_type_args,
                        arg_exprs: call.args,
                        span: call.span,
                        expected_result: expected,
                        argument_protocol: CallArgumentProtocol::Ordinary,
                    },
                ) {
                    Ok(probe) => {
                        probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe)));
                        setups.push(input.property.sink.clone());
                    }
                    Err(failure) => {
                        first_failure.get_or_insert(failure);
                    }
                }
            }
        }
        if probes.is_empty() {
            return match (suppressed, first_failure) {
                (true, Some(failure)) => PropertyExtensionInvokeOutcome::Failed(failure),
                (true, None) => PropertyExtensionInvokeOutcome::Blocked,
                (false, failure) => PropertyExtensionInvokeOutcome::NoApplicable(failure),
            };
        }

        let mut state = self.clone();
        let Some(winner) =
            state.select_named_function_like("invoke", layer_name, &probes, call.args, call.span)
        else {
            return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
        };
        let probe = probes.swap_remove(winner);
        let mut layer_sink = setups.swap_remove(winner);
        let NamedFunctionLikeProbe::Callable(probe) = probe else {
            unreachable!("property extension invoke probes contain only callable candidates")
        };
        let resolved = state.commit_named_callable(*probe, &mut layer_sink);
        let callee = state.materialize_resolved_callee(&resolved);
        state.check_call_effects(callee, call.span);
        let expression = hir::Expr {
            kind: hir::ExprKind::Call {
                callee,
                args: resolved.args,
            },
            ty: resolved.return_ty,
            span: call.span,
            origin: state.expression_origin(call.span),
        };
        PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
            state: Box::new(state),
            expression,
            sink: layer_sink,
        })
    }

    pub(crate) fn lower_named_call_on_receiver(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
    ) -> Option<hir::Expr> {
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::Function(_)) {
            if required.operator.is_some()
                || required.property_delegate_operator.is_some()
                || required.infix
            {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!(
                        "type `{found}` has no matching callable role `{}`",
                        name.text
                    ),
                );
                return None;
            }
            if !call.type_args.is_empty() {
                self.error(
                    name.span,
                    "function values do not accept explicit type arguments".to_string(),
                );
                return None;
            }
            return self.lower_callable_call(receiver, call.args, call.span, sink);
        }
        let mut candidates = match required.operator {
            Some(operator) => self.methods_by_operator(receiver.ty, operator),
            None => self.methods_by_name(receiver.ty, &name.text),
        };
        candidates.retain(|candidate| {
            let modifiers = self.signatures[&candidate.function].modifiers;
            Self::matches_required_modifiers(modifiers, required)
        });
        let mut first_failure = None;
        if !candidates.is_empty() && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = candidates.len();
            candidates.retain(|candidate| {
                let sig = &self.signatures[&candidate.function];
                sig.type_params.len() == sig.owner_type_param_count
            });
            if candidates.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                let mut failure = self.clone();
                failure.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be called through interface type `{found}`",
                        name.text
                    ),
                );
                first_failure = Some(Box::new(failure));
            }
        }
        if !candidates.is_empty() {
            match self.probe_member_call_partition(
                candidates,
                &name.text,
                receiver.clone(),
                call,
                expected,
                required.operator == Some(hir::OperatorKind::Set),
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    first_failure = failure;
                }
            }
        }

        let extension_layers = match required.operator {
            Some(operator) => self.named_extension_operator_layers(operator),
            None => self.named_extension_call_layers(&name.text),
        };
        for layer in extension_layers {
            let suppressed = layer.suppressed_callables.iter().copied().any(|function| {
                Self::matches_required_modifiers(self.signatures[&function].modifiers, required)
            });
            let extensions = layer
                .candidates
                .into_iter()
                .filter(|function| {
                    let modifiers = self.signatures[function].modifiers;
                    Self::matches_required_modifiers(modifiers, required)
                })
                .collect::<Vec<_>>();
            if extensions.is_empty() {
                if suppressed {
                    return None;
                }
                continue;
            }
            match self.probe_extension_call_partition(
                &extensions,
                &name.text,
                receiver.clone(),
                call,
                expected,
                required.operator == Some(hir::OperatorKind::Set),
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    if let Some(failure) = failure {
                        if suppressed {
                            self.commit_layer_diagnostics(*failure);
                            return None;
                        }
                        first_failure.get_or_insert(failure);
                    } else if suppressed {
                        return None;
                    }
                }
            }
        }

        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
        } else {
            let found = self.type_name(receiver.ty);
            self.error(
                name.span,
                format!("type `{found}` has no method `{}`", name.text),
            );
        }
        None
    }

    pub(super) fn lower_infix_call(
        &mut self,
        lhs: &ast::Expr,
        target: &ast::InfixTarget,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let args = [ast::CallArgument::positional(rhs.clone())];
        let call = CallSite {
            type_args: &[],
            args: &args,
            span,
        };
        if let ast::InfixTarget::Named(name) = target
            && let Some(layer) =
                self.probe_integer_literal_receiver(lhs, expected, |state, receiver, layer_sink| {
                    state.lower_explicit_named_call(
                        receiver,
                        name,
                        call,
                        layer_sink,
                        expected,
                        RequiredCallableModifiers {
                            operator: None,
                            infix: true,
                            ..Default::default()
                        },
                    )
                })
        {
            return Some(self.commit_expr_layer(layer, sink));
        }
        let receiver = self.lower_expr(lhs, sink, None)?;
        match target {
            ast::InfixTarget::Named(name) => self.lower_explicit_named_call(
                receiver,
                name,
                call,
                sink,
                expected,
                RequiredCallableModifiers {
                    operator: None,
                    infix: true,
                    ..Default::default()
                },
            ),
            ast::InfixTarget::Invoke => {
                self.lower_value_invoke(receiver, call, sink, expected, true)
            }
        }
    }

    pub(super) fn lower_safe_method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(receiver, sink, None)?;
        let Some(inner) = self.as_option(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                call.span,
                format!("`?.` requires an Option receiver, found {found}"),
            );
            return None;
        };
        let option_ty = receiver.ty;
        let origin = self.expression_origin(call.span);
        let receiver_local = self.alloc_hidden("opt", option_ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding {
                    local: receiver_local,
                },
                init: receiver,
            },
            span: call.span,
        });
        let receiver_ref = hir::Expr {
            kind: ExprKind::Local(receiver_local),
            ty: option_ty,
            span: call.span,
            origin,
        };
        let payload = hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(receiver_ref.clone()),
                trap_on_none: false,
            },
            ty: inner,
            span: call.span,
            origin,
        };
        let mut then_body = Vec::new();
        let value = self.lower_explicit_named_call(
            payload,
            name,
            call,
            &mut then_body,
            expected.and_then(|ty| self.as_option(ty)),
            RequiredCallableModifiers::default(),
        )?;
        let result_ty = self.option_type(value.ty);
        let result = self.alloc_hidden("res", result_ty);
        then_body.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: hir::Expr {
                    kind: ExprKind::SomeWrap(Box::new(value)),
                    ty: result_ty,
                    span: call.span,
                    origin,
                },
            },
            span: call.span,
        });
        let else_body = vec![hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: hir::Expr {
                    kind: ExprKind::NoneLiteral,
                    ty: result_ty,
                    span: call.span,
                    origin,
                },
            },
            span: call.span,
        }];
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: hir::Expr {
                    kind: ExprKind::IsSome(Box::new(receiver_ref)),
                    ty: self.boolean,
                    span: call.span,
                    origin,
                },
                then_body,
                else_body: Some(else_body),
            },
            span: call.span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: result_ty,
            span: call.span,
            origin,
        })
    }
}
