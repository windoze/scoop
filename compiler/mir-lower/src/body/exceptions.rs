//! Construct runtime exceptions through local or dependency initializers.

use super::*;

impl BodyLowerer<'_> {
    /// Construct and throw one compiler-known exception. The zero-argument
    /// constructor target is complete in LocalConcrete HIR, so this operation
    /// only transposes typed identities.
    pub(super) fn throw_builtin(
        &mut self,
        exception: hir::CompilerException,
        span: Span,
    ) -> smir::Statement {
        let constructor = &self.module.class_constructors[exception.callable()];
        debug_assert!(constructor.parameters.is_empty());
        let ctor = self.ctors[&exception.callable()];
        let class_id = self.class_map[&constructor.class];
        self.throw_class(class_id, mir::Callee::User(ctor), span)
    }

    pub(super) fn throw_class(
        &self,
        class_id: mir::ClassId,
        initializer: mir::Callee,
        span: Span,
    ) -> smir::Statement {
        let exception_ty = mir::Type::Class(class_id);
        smir::Statement {
            kind: smir::StatementKind::Throw(smir::Expr::new(
                exception_ty.clone(),
                smir::ExprKind::ClassNew {
                    class_id,
                    initializer,
                    args: Vec::new(),
                },
            )),
            span,
        }
    }

    pub(super) fn throw_imported_exception(
        &self,
        declaration: scoop_identity::PersistentTypeId,
        constructor: &scoop_hir::ImportedCoreProtocolCallable,
        span: Span,
    ) -> smir::Statement {
        let (provider, target) = crate::current::runtime_constructor_target(constructor)
            .expect("the frontend retained a typed exception constructor");
        let callable = self
            .external_callables
            .iter()
            .find_map(|(id, callable)| {
                (callable.reference().provider() == provider
                    && callable.reference().implementation() == target)
                    .then_some(id)
            })
            .expect("the required constructor was selected from its provider");
        let class = self
            .module
            .classes
            .iter()
            .find_map(|(id, class)| {
                (class.origin.concrete_type_id() == Some(declaration)).then_some(id)
            })
            .expect("the HIR stage completed the required exception representation");
        self.throw_class(
            self.class_map[&class],
            mir::Callee::External(callable),
            span,
        )
    }
}
