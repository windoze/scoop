use crate::Lowerer;
use scoop_hir as hir;

use super::{PointeeRequirementCallSite, RequirementContext};

impl Lowerer {
    pub(super) fn pointee_requirement_call_sites(&mut self) -> Vec<PointeeRequirementCallSite> {
        let mut out = Vec::new();
        for (caller, function) in self.functions.iter() {
            let hir::FunctionKind::User(body) = &function.kind else {
                continue;
            };
            let file = self
                .function_files
                .get(&caller)
                .copied()
                .unwrap_or_else(|| self.primary_output_file());
            push_pointee_call_sites(
                &mut out,
                RequirementContext::Function(caller),
                file,
                self.generic_calls_in_body(body),
            );
        }

        for (id, declaration) in self.structs.iter() {
            let context = RequirementContext::Nominal(crate::Owner::Struct(id));
            let file = self.struct_files[&id];
            for constructor in &declaration.constructors {
                push_pointee_call_sites(
                    &mut out,
                    context,
                    file,
                    self.generic_calls_in_struct_constructor(
                        &self.struct_constructors[*constructor],
                    ),
                );
            }
        }
        for (id, declaration) in self.classes.iter() {
            let context = RequirementContext::Nominal(crate::Owner::Class(id));
            let file = self.class_files[&id];
            for constructor in &declaration.constructors {
                push_pointee_call_sites(
                    &mut out,
                    context,
                    file,
                    self.generic_calls_in_class_constructor(&self.class_constructors[*constructor]),
                );
            }
        }

        let mut templates = Vec::new();
        for ((owner, _), template) in &self.default_templates {
            if !templates.contains(&(*owner, *template)) {
                templates.push((*owner, *template));
            }
        }
        templates.sort_by_key(|(owner, template)| {
            (
                default_owner_sort_key(*owner),
                default_template_sort_key(*template),
            )
        });
        for (owner, template) in templates {
            let (body, bindings) = match template {
                crate::defaults::DefaultExprTemplateRef::Local(template) => {
                    (self.local_default_exprs[template].body.clone(), Vec::new())
                }
                crate::defaults::DefaultExprTemplateRef::Export(source) => {
                    let source = self.export_default_sources[source].clone();
                    let Some((expression, type_arguments)) = source.declared() else {
                        // Imported parameter-free bodies have no local generic applications.
                        continue;
                    };
                    let body = self.export_default_exprs[expression].clone();
                    let bindings = body
                        .type_parameters
                        .iter()
                        .copied()
                        .zip(type_arguments.iter().copied())
                        .collect();
                    (body, bindings)
                }
            };
            let context = requirement_context_for_default(self, owner);
            let mut calls = self.generic_calls_in_default(&body);
            if !bindings.is_empty() {
                for call in &mut calls {
                    for (_, argument) in &mut call.arguments {
                        *argument = self.instantiate_method_ty(*argument, &bindings);
                    }
                }
            }
            push_pointee_call_sites(&mut out, context, body.origin.file as usize, calls);
        }
        out
    }
}

fn push_pointee_call_sites(
    out: &mut Vec<PointeeRequirementCallSite>,
    context: RequirementContext,
    file: usize,
    calls: impl IntoIterator<Item = super::super::no_gc_generics::GenericCall>,
) {
    // Constructor pointee requirements belong to their nominal applications;
    // the nominal occurrence pass already validates and propagates that edge.
    out.extend(calls.into_iter().filter_map(|call| {
        let callee = match call.callee {
            callee @ (super::super::no_gc_generics::GenericCallable::Function(_)
            | super::super::no_gc_generics::GenericCallable::Imported(_)
            | super::super::no_gc_generics::GenericCallable::ImportedConstructor(_)) => callee,
            super::super::no_gc_generics::GenericCallable::ClassConstructor(_)
            | super::super::no_gc_generics::GenericCallable::StructConstructor(_) => return None,
        };
        Some(PointeeRequirementCallSite {
            context,
            callee,
            arguments: call.arguments,
            file,
            span: call.span,
        })
    }));
}

pub(super) fn requirement_context_for_default(
    lowerer: &Lowerer,
    owner: crate::defaults::SourceParameterOwner,
) -> RequirementContext {
    match owner {
        crate::defaults::SourceParameterOwner::Function(function) => {
            RequirementContext::Function(function)
        }
        crate::defaults::SourceParameterOwner::StructConstructor(constructor) => {
            RequirementContext::Nominal(crate::Owner::Struct(
                lowerer.struct_constructors[constructor].owner,
            ))
        }
        crate::defaults::SourceParameterOwner::ClassConstructor(constructor) => {
            RequirementContext::Nominal(crate::Owner::Class(
                lowerer.class_constructors[constructor].owner,
            ))
        }
        crate::defaults::SourceParameterOwner::VariantConstructor(variant) => {
            RequirementContext::Nominal(crate::Owner::Enum(variant.enumeration()))
        }
    }
}

pub(super) fn default_owner_sort_key(
    owner: crate::defaults::SourceParameterOwner,
) -> (u8, u32, u32) {
    match owner {
        crate::defaults::SourceParameterOwner::Function(id) => (0, id.into_raw().into_u32(), 0),
        crate::defaults::SourceParameterOwner::StructConstructor(id) => {
            (1, id.into_raw().into_u32(), 0)
        }
        crate::defaults::SourceParameterOwner::ClassConstructor(id) => {
            (2, id.into_raw().into_u32(), 0)
        }
        crate::defaults::SourceParameterOwner::VariantConstructor(variant) => (
            3,
            variant.enumeration().into_raw().into_u32(),
            variant.local_index(),
        ),
    }
}

pub(super) fn default_template_sort_key(
    template: crate::defaults::DefaultExprTemplateRef,
) -> (u8, u32) {
    match template {
        crate::defaults::DefaultExprTemplateRef::Local(id) => (0, id.into_raw().into_u32()),
        crate::defaults::DefaultExprTemplateRef::Export(id) => (1, id.into_raw().into_u32()),
    }
}
