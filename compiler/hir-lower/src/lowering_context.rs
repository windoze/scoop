use super::*;

impl Lowerer {
    /// Establish one lexical source boundary for expression provenance.
    /// Contexts are interned by source and typed subject so arena allocation
    /// order and repeated visits cannot create distinct semantic contexts.
    pub(crate) fn set_source_context(&mut self, subject: hir::SourceContextSubject) {
        self.current_source_context = Some(self.intern_source_context(subject));
    }

    fn intern_source_context(
        &mut self,
        subject: hir::SourceContextSubject,
    ) -> hir::SourceContextId {
        let context = hir::SourceContext::new(
            self.intrinsic_sources[self.current_file].identity.clone(),
            subject,
        );
        self.source_context_by_value
            .get(&context)
            .copied()
            .unwrap_or_else(|| {
                let id = self.source_contexts.alloc(context.clone());
                self.source_context_by_value.insert(context, id);
                id
            })
    }

    /// Interning a context never allocates constructors, so the next arena
    /// slot is also the exact typed subject of the following allocation.
    pub(crate) fn next_class_constructor_context(&mut self) -> hir::SourceContextId {
        let index = u32::try_from(self.class_constructors.len())
            .expect("constructor arena indices fit in u32");
        let constructor = hir::ClassConstructorId::from_raw(index.into());
        self.intern_source_context(hir::SourceContextSubject::Constructor(
            hir::SourceContextConstructor::Class(constructor),
        ))
    }

    pub(crate) fn source_context_for_current_file(&self) -> hir::SourceContextId {
        let source = &self.intrinsic_sources[self.current_file].identity;
        self.current_source_context
            .filter(|&context| self.source_contexts[context].source() == source)
            .unwrap_or(self.file_source_contexts[self.current_file])
    }

    /// Allocate a hidden desugaring temporary (`$opt.N` / `$res.N`).
    /// The `$` prefix keeps it out of the source namespace (the parser
    /// never produces `$` identifiers), so it is not registered in
    /// `scopes`; generated code references it by `LocalId` directly.
    pub(crate) fn alloc_hidden(&mut self, prefix: &str, ty: TypeId) -> hir::LocalId {
        self.alloc_hidden_with_role(prefix, ty, scoop_identity::SyntheticLocalRole::Temporary)
    }

    pub(crate) fn alloc_desugared_iterator_hidden(
        &mut self,
        prefix: &str,
        ty: TypeId,
    ) -> hir::LocalId {
        self.alloc_hidden_with_role(
            prefix,
            ty,
            scoop_identity::SyntheticLocalRole::DesugaredIterator,
        )
    }

    fn alloc_hidden_with_role(
        &mut self,
        prefix: &str,
        ty: TypeId,
        role: scoop_identity::SyntheticLocalRole,
    ) -> hir::LocalId {
        let name = format!("${prefix}.{}", self.hidden_count);
        self.hidden_count += 1;
        self.alloc_synthetic_local(name, ty, false, role)
    }

    /// Allocate the branch-result local used when a structured control
    /// expression is lowered into statements. Every normally completing
    /// branch assigns it before the resulting local read is reachable.
    pub(crate) fn alloc_hidden_result(&mut self, ty: TypeId) -> hir::LocalId {
        let name = format!("$result.{}", self.hidden_count);
        self.hidden_count += 1;
        self.alloc_synthetic_local(
            name,
            ty,
            true,
            scoop_identity::SyntheticLocalRole::Temporary,
        )
    }

    /// The variant index of `name` in `enum_id`, if it exists.
    pub(crate) fn find_variant(&self, enum_id: EnumId, name: &str) -> Option<u32> {
        self.enums[enum_id]
            .variants
            .iter()
            .position(|v| v.name == name)
            .map(|index| index as u32)
    }

    /// A checked typed variant identity for one declaration-local name.
    pub(crate) fn find_variant_ref(
        &self,
        enum_id: EnumId,
        name: &str,
    ) -> Option<hir::EnumVariantRef> {
        let index = self.find_variant(enum_id, name)?;
        hir::EnumVariantRef::checked(&self.enums, enum_id, index)
    }

    /// Ordinary core-prelude candidates for a source name. The table is
    /// populated exclusively by core-contract validation.
    pub(crate) fn core_prelude_variant_refs(&self, name: &str) -> &[hir::EnumVariantRef] {
        self.core_prelude_variants.get(name)
    }

    pub(crate) fn resolved_variant_style(&self, target: hir::EnumVariantRef) -> VariantStyle {
        self.enums[target.enumeration()].variants[target.local_index() as usize].style
    }

    /// Resolve the lowest-priority contextual layer against one exact enum
    /// application. This never scans unrelated enum declarations.
    pub(crate) fn contextual_variant_ref(
        &self,
        name: &str,
        expected: Option<TypeId>,
    ) -> Option<hir::EnumVariantRef> {
        let Type::Enum(application) = self.types[*expected.as_ref()?] else {
            return None;
        };
        let enumeration = self.enum_applications[application].template;
        let crate::Owner::Enum(enumeration) = self.nominal_owners.get(&enumeration)? else {
            unreachable!("an enum application retains its enum declaration")
        };
        self.find_variant_ref(*enumeration, name)
    }

    pub(crate) fn exact_expected_enum(&self, expected: Option<TypeId>) -> Option<EnumId> {
        let Type::Enum(application) = self.types[*expected.as_ref()?] else {
            return None;
        };
        let crate::Owner::Enum(enumeration) = self
            .nominal_owners
            .get(&self.enum_applications[application].template)?
        else {
            unreachable!("an enum application retains its enum declaration")
        };
        Some(*enumeration)
    }

    /// During declaration/type pass 1 this reads the provisional core enum;
    /// after pass 2 only the complete checked Option contract remains.
    pub(crate) fn option_enumeration(&self) -> Option<EnumId> {
        self.option_core
            .map(hir::OptionCore::enumeration)
            .or(self.pending_option_enum)
    }

    pub(crate) fn has_option_protocol(&self) -> bool {
        matches!(self.core, crate::CoreLoweringAuthority::Imported(_))
            || self.option_enumeration().is_some()
    }

    /// Whether `ty` is `Option<T>`; returns `T`.
    pub(crate) fn as_option(&self, ty: TypeId) -> Option<TypeId> {
        let Type::Enum(application) = self.types[ty] else {
            return None;
        };
        let application = &self.enum_applications[application];
        let [argument] = application.arguments.as_slice() else {
            return None;
        };
        let option = match &self.core {
            crate::CoreLoweringAuthority::Imported(protocols) => {
                hir::SourceNominalId::GenericTemplate(protocols.option().option().persistent())
            }
            _ => self
                .nominal_identity(crate::Owner::Enum(self.option_enumeration()?))
                .declaration_id(),
        };
        (application.template == option).then_some(*argument)
    }

    /// `Option<inner>` (interned). Only called when the core `Option`
    /// validated successfully.
    pub(crate) fn option_type(&mut self, inner: TypeId) -> TypeId {
        if let crate::CoreLoweringAuthority::Imported(protocols) = &self.core {
            let owner = protocols.option().option().persistent();
            return self
                .imported_nominal_application(
                    hir::SourceNominalId::GenericTemplate(owner),
                    vec![inner],
                )
                .expect("the checked Option protocol has a complete dependency declaration");
        }
        let id = self
            .option_enumeration()
            .expect("Option types only exist after core validation");
        self.enum_application(id, vec![inner])
    }

    pub(crate) fn push_suspension_context(&mut self, context: SuspensionContext) {
        self.suspension_contexts.push(context);
    }

    pub(crate) fn pop_suspension_context(&mut self) {
        assert!(
            self.suspension_contexts.len() > 1,
            "the root suspension context must remain present"
        );
        self.suspension_contexts.pop();
    }

    pub(crate) fn push_safety_context(&mut self, safety: hir::Safety) {
        self.safety_contexts.push(safety);
    }

    pub(crate) fn pop_safety_context(&mut self) {
        assert!(
            self.safety_contexts.len() > 1,
            "the root safety context must remain present"
        );
        self.safety_contexts.pop();
    }

    /// Diagnose a selected suspend call in a body whose ABI has no continuation.
    pub(crate) fn check_suspend_context(&mut self, callee: &str, span: Span) {
        let context = *self
            .suspension_contexts
            .last()
            .expect("the suspension context stack is initialized non-empty");
        let SuspensionContext::Forbidden(reason) = context else {
            return;
        };
        let location = match reason {
            ForbiddenSuspendContext::TopLevel => "a non-suspend declaration".to_string(),
            ForbiddenSuspendContext::Function => {
                format!("non-suspend function `{}`", self.current_fn_name)
            }
            ForbiddenSuspendContext::DefaultExpression => {
                format!("non-suspend default expression {}", self.current_fn_name)
            }
            ForbiddenSuspendContext::ConstructorDelegation => "constructor delegation".to_string(),
            ForbiddenSuspendContext::ConstructorInitialization => {
                "constructor initialization".to_string()
            }
        };
        self.error(
            span,
            format!("suspend function `{callee}` cannot be called from {location}"),
        );
    }

    pub(crate) fn check_call_effects(&mut self, callable: hir::Callable, span: Span) {
        let function = self.callable_function_id(callable);
        if self.functions[function].is_suspend {
            self.check_suspend_context(&self.functions[function].name.clone(), span);
        }
        if self.functions[function].attributes.safety != hir::Safety::Unsafe {
            return;
        }
        let context = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized non-empty");
        if context == hir::Safety::Safe {
            self.error(
                span,
                format!(
                    "unsafe function `{}` may only be called from an unsafe context",
                    self.functions[function].name
                ),
            );
        }
    }

    pub(crate) fn require_unsafe_operation(&mut self, span: Span, operation: &str) -> bool {
        let context = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized non-empty");
        if context == hir::Safety::Unsafe {
            return true;
        }
        self.error(span, format!("{operation} requires an unsafe context"));
        false
    }

    pub(crate) fn error(&mut self, span: Span, message: String) {
        let mut diagnostic = Diagnostic::at(span, message);
        diagnostic.file = self.current_file;
        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn warning(&mut self, span: Span, message: String) {
        let mut diagnostic = Diagnostic::warning_at(span, message);
        diagnostic.file = self.current_file;
        self.warnings.push(diagnostic);
    }
}
