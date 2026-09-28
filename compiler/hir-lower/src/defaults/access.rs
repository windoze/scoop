use scoop_hir as hir;
use std::collections::HashSet;

use crate::Lowerer;

mod bindings;
mod coverage;
mod expressions;
mod shapes;
mod statements;

struct ReferenceCollector<'a> {
    lowerer: &'a mut Lowerer,
    references: hir::ExportDefaultReferences,
    call_domain: hir::CallDomain,
    fallback_origin: hir::DefinitionOrigin,
    local_declarations: HashSet<hir::FunctionId>,
}

impl Lowerer {
    pub(super) fn collect_export_default_references(
        &mut self,
        owner: hir::ExportParameterOwner,
        template: &hir::ExportDefaultExpr,
    ) -> hir::ExportDefaultReferences {
        let call_domain = self.default_call_domain(owner);
        let mut collector = ReferenceCollector {
            lowerer: self,
            references: hir::ExportDefaultReferences::default(),
            call_domain,
            fallback_origin: template.origin,
            local_declarations: HashSet::new(),
        };
        let mut locals = template.locals.values().collect::<Vec<_>>();
        locals.sort_unstable_by(|left, right| left.selector.cmp(&right.selector));
        for local in locals {
            let origin = match local.definition {
                hir::LocalValueDefinitionSite::Source(origin) => origin,
                hir::LocalValueDefinitionSite::Synthetic => template.origin,
            };
            collector.type_reference(local.ty, origin);
        }
        collector.statements(&template.statements);
        collector.expression(&template.value);
        collector.references
    }
}

impl ReferenceCollector<'_> {
    fn direct_delegate_storage(&mut self, origin: hir::DefinitionOrigin) {
        self.lowerer.error(
            origin.span,
            "default expressions must access delegated properties through accessors".to_string(),
        );
    }

    fn checked_target_domain(
        &mut self,
        target_domain: hir::AccessDomain,
        origin: hir::DefinitionOrigin,
        target_kind: &str,
    ) -> hir::AccessDomain {
        self.lowerer.check_default_reference_access(
            &self.call_domain,
            &target_domain,
            origin,
            target_kind,
        );
        target_domain
    }

    fn callable_domain(&self, callable: hir::Callable) -> hir::AccessDomain {
        let function = self.lowerer.callable_function_id(callable);
        if self.local_declarations.contains(&function) {
            // This definition is carried by the default body itself.
            hir::AccessDomain::universal()
        } else {
            self.lowerer.function_access_domain(function)
        }
    }

    fn method_callee_domain(&self, callee: hir::MethodCallee) -> hir::AccessDomain {
        let function = match callee {
            hir::MethodCallee::Callable(callable) => self.lowerer.callable_function_id(callable),
            hir::MethodCallee::Bound(bound) => match self.lowerer.bound_callable_refs[bound].source
            {
                hir::BoundCallableSource::Class { callable, .. } => {
                    self.lowerer.callable_function_id(callable)
                }
                hir::BoundCallableSource::Interface { member, .. } => {
                    self.lowerer.interface_method_entities[member].function
                }
            },
            hir::MethodCallee::DerivedEquality(application) => {
                let application = &self.lowerer.derived_equality_applications[application];
                match application.origin {
                    hir::DerivedEqualityOrigin::Nominal(_) => application.function,
                    hir::DerivedEqualityOrigin::Structural(owner_type) => {
                        return self.lowerer.type_access_domain(owner_type);
                    }
                }
            }
        };
        self.lowerer.function_access_domain(function)
    }

    fn callable_target_domain(
        &self,
        target: &hir::ExportDefaultCallableTarget,
    ) -> hir::AccessDomain {
        match *target {
            hir::ExportDefaultCallableTarget::Callable(callable) => self.callable_domain(callable),
            hir::ExportDefaultCallableTarget::ImportedDependency(_)
            | hir::ExportDefaultCallableTarget::ImportedGeneric(_) => {
                hir::AccessDomain::universal()
            }
            hir::ExportDefaultCallableTarget::Bound(bound) => {
                self.method_callee_domain(hir::MethodCallee::Bound(bound))
            }
            hir::ExportDefaultCallableTarget::DerivedEquality(application) => {
                self.method_callee_domain(hir::MethodCallee::DerivedEquality(application))
            }
            hir::ExportDefaultCallableTarget::FunctionAddress(function) => {
                self.lowerer.function_access_domain(function)
            }
            hir::ExportDefaultCallableTarget::CallableReference(reference) => {
                match &self.lowerer.callable_references[reference].target {
                    hir::CallableReferenceTarget::Named(callable) => {
                        self.callable_domain(*callable)
                    }
                    hir::CallableReferenceTarget::BoundMember { callee, .. } => {
                        self.method_callee_domain(*callee)
                    }
                    hir::CallableReferenceTarget::BoundExtension { callee, .. } => {
                        self.callable_domain(*callee)
                    }
                    hir::CallableReferenceTarget::Local { .. } => hir::AccessDomain::universal(),
                }
            }
            hir::ExportDefaultCallableTarget::LocalFunction(_)
            | hir::ExportDefaultCallableTarget::Lambda(_)
            | hir::ExportDefaultCallableTarget::AnonymousFunction(_) => {
                hir::AccessDomain::universal()
            }
        }
    }

    pub(super) fn record_callable(
        &mut self,
        target: hir::ExportDefaultCallableTarget,
        origin: hir::DefinitionOrigin,
    ) {
        let target_domain = self.callable_target_domain(&target);
        let target_domain = self.checked_target_domain(target_domain, origin, "a callable");
        self.references
            .callables
            .push(hir::ExportDefaultCallableRef {
                target,
                target_domain,
                origin,
            });
    }

    pub(super) fn record_constructor(
        &mut self,
        target: hir::ExportDefaultConstructorTarget,
        origin: hir::DefinitionOrigin,
    ) {
        let target_domain = self.lowerer.constructor_access_domain(target);
        let target_domain = self.checked_target_domain(target_domain, origin, "a constructor");
        self.references
            .constructors
            .push(hir::ExportDefaultConstructorRef {
                target,
                target_domain,
                origin,
            });
    }

    pub(super) fn type_reference(&mut self, target: hir::TypeId, origin: hir::DefinitionOrigin) {
        if matches!(self.lowerer.types[target], hir::Type::Param(_)) {
            return;
        }
        let target_domain = self.lowerer.type_access_domain(target);
        let target_domain = self.checked_target_domain(target_domain, origin, "a type");
        self.references.types.push(hir::ExportDefaultTypeRef {
            target: hir::ExportDefaultTypeTarget::Type(target),
            target_domain,
            origin,
        });
    }

    pub(super) fn global(&mut self, target: hir::GlobalId, origin: hir::DefinitionOrigin) {
        let property = self.lowerer.globals[target].property;
        let target_domain = self.lowerer.properties[property].access.lookup.0.clone();
        let target_domain = self.checked_target_domain(target_domain, origin, "a property");
        self.references.globals.push(hir::ExportDefaultGlobalRef {
            target,
            target_domain,
            origin,
        });
    }

    pub(super) fn singleton_value(
        &mut self,
        target: hir::ExportDefaultSingletonTarget,
        origin: hir::DefinitionOrigin,
    ) {
        // The expression's type reference already covers the object's shared visibility.
        self.references
            .singleton_values
            .push(hir::ExportDefaultSingletonValueRef { target, origin });
    }

    pub(super) fn record_field(&mut self, target: hir::FieldRef, origin: hir::DefinitionOrigin) {
        let target_domain = self.lowerer.field_access_domain(target);
        let target_domain = self.checked_target_domain(target_domain, origin, "a field");
        self.references.fields.push(hir::ExportDefaultFieldRef {
            target,
            target_domain,
            origin,
        });
    }

    pub(super) fn at(&self, span: scoop_ast::Span) -> hir::DefinitionOrigin {
        hir::DefinitionOrigin {
            provider: self.fallback_origin.provider,
            file: self.fallback_origin.file,
            span,
            context: self.fallback_origin.context,
        }
    }
}
