use std::collections::HashSet;

use scoop_hir as hir;

use crate::Lowerer;

use super::call_sites::{
    default_owner_sort_key, default_template_sort_key, requirement_context_for_default,
};
use super::occurrences::{
    TypeOccurrence, collect_body_type_occurrences, collect_class_constructor_type_occurrences,
    collect_default_type_occurrences, collect_expr_types, collect_statement_types,
    collect_struct_constructor_type_occurrences,
};
use super::{PointeeApplicationOccurrence, PointeeRequirementCallSite, RequirementContext};

impl Lowerer {
    pub(crate) fn validate_gc_free_pointee_requirements(&mut self) {
        let default_occurrences = self.default_pointee_application_occurrences();
        let concrete_default_occurrences = self.default_concrete_pointee_application_occurrences();
        let call_sites = self.pointee_requirement_call_sites();
        loop {
            let nominal_changed =
                self.infer_nominal_gc_free_pointee_requirements(&default_occurrences);
            let callable_changed =
                self.infer_callable_gc_free_pointee_requirements(&default_occurrences);
            let call_changed = self.propagate_callable_gc_free_pointee_requirements(&call_sites);
            if !nominal_changed && !callable_changed && !call_changed {
                break;
            }
        }
        self.validate_callable_gc_free_pointee_requirements(&call_sites);
        self.validate_pointer_type_uses();
        self.validate_concrete_pointee_applications(concrete_default_occurrences);
    }

    fn validate_callable_gc_free_pointee_requirements(
        &mut self,
        call_sites: &[PointeeRequirementCallSite],
    ) {
        for call_site in call_sites {
            let requirements = self
                .callable_gc_free_pointee_requirements(call_site.callee)
                .to_vec();
            for requirement in requirements {
                let argument = call_site.argument(requirement.type_param);
                let allowed = self.allowed_pointee_parameters(call_site.context);
                if self.gc_free_requirements(argument).is_some_and(|mapped| {
                    mapped.iter().all(|parameter| allowed.contains(parameter))
                }) {
                    continue;
                }
                self.current_file = call_site.file;
                self.error(
                    call_site.span,
                    format!(
                        "generic function `{}` requires type argument {} for `{}` to be a GC-free `Ptr` pointee",
                        self.effect_callable_name(call_site.callee),
                        self.type_name(argument),
                        self.effect_callable_parameter(call_site.callee, requirement.type_param).name
                    ),
                );
            }
        }
    }

    fn validate_concrete_pointee_applications(
        &mut self,
        mut occurrences: Vec<PointeeApplicationOccurrence>,
    ) {
        occurrences.extend(self.pointee_application_occurrences());
        let mut reported = HashSet::new();
        for PointeeApplicationOccurrence {
            ty,
            file,
            span,
            context,
        } in occurrences
        {
            // Direct `Ptr<P>` syntax is owned by `pointer_type_uses`, which
            // reports the precise pointee span. This replay pass validates
            // applications that inherit a requirement through a generic
            // nominal/callable; re-reporting a root pointer here would emit
            // the same failed predicate once for its signature and again for
            // the body's parameter local.
            let replay_ty = match self.types[ty] {
                hir::Type::Ptr(pointee) => pointee,
                _ => ty,
            };
            let mut requirements = HashSet::new();
            self.collect_pointee_parameters(replay_ty, &mut HashSet::new(), &mut requirements);
            let allowed = match context {
                RequirementContext::Function(function) => self
                    .function_gc_free_pointee_requirements(function)
                    .iter()
                    .map(|requirement| requirement.type_param)
                    .collect::<HashSet<_>>(),
                RequirementContext::Nominal(owner) => self
                    .nominal_gc_free_pointee_requirements(owner)
                    .iter()
                    .map(|requirement| requirement.type_param)
                    .collect::<HashSet<_>>(),
                RequirementContext::Closed => HashSet::new(),
            };
            if requirements
                .iter()
                .all(|parameter| allowed.contains(parameter))
                && self.pointee_type_is_valid(replay_ty, &mut HashSet::new())
            {
                continue;
            }
            if !reported.insert((replay_ty.into_raw().into_u32(), file, span.start, span.end)) {
                continue;
            }
            self.current_file = file;
            self.error(
                span,
                format!(
                    "type {} requires a GC-free `Ptr` pointee argument",
                    self.type_name(replay_ty)
                ),
            );
        }
    }

    fn pointee_type_is_valid(&self, ty: hir::TypeId, visiting: &mut HashSet<hir::TypeId>) -> bool {
        if !visiting.insert(ty) {
            return true;
        }
        let valid = match &self.types[ty] {
            // `pointer_type_uses` owns the direct `Ptr<P>` predicate at its
            // precise source span. This traversal only replays requirements
            // inherited by applications nested inside `P`.
            hir::Type::Ptr(pointee) => self.pointee_type_is_valid(*pointee, visiting),
            hir::Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                self.application_pointee_is_valid(
                    &self.structs[self.struct_id(application.template)].type_params,
                    &self.structs[self.struct_id(application.template)]
                        .gc_free_pointee_requirements,
                    &application.arguments,
                ) && application
                    .arguments
                    .iter()
                    .all(|argument| self.pointee_type_is_valid(*argument, visiting))
            }
            hir::Type::Class(application) => {
                let application = &self.class_applications[*application];
                self.application_pointee_is_valid(
                    &self.classes[self.class_id(application.template)].type_params,
                    &self.classes[self.class_id(application.template)].gc_free_pointee_requirements,
                    &application.arguments,
                ) && application
                    .arguments
                    .iter()
                    .all(|argument| self.pointee_type_is_valid(*argument, visiting))
            }
            hir::Type::Interface(application) => {
                let application = &self.interface_applications[*application];
                self.application_pointee_is_valid(
                    &self.interfaces[self.interface_id(application.template)].type_params,
                    &self.interfaces[self.interface_id(application.template)]
                        .gc_free_pointee_requirements,
                    &application.arguments,
                ) && application
                    .arguments
                    .iter()
                    .all(|argument| self.pointee_type_is_valid(*argument, visiting))
            }
            hir::Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                self.source_enum_id(application.template).is_none_or(|id| {
                    self.application_pointee_is_valid(
                        &self.enums[id].type_params,
                        &self.enums[id].gc_free_pointee_requirements,
                        &application.arguments,
                    )
                }) && application
                    .arguments
                    .iter()
                    .all(|argument| self.pointee_type_is_valid(*argument, visiting))
            }
            hir::Type::Tuple(elements) => elements
                .iter()
                .all(|element| self.pointee_type_is_valid(*element, visiting)),
            hir::Type::Function(function) | hir::Type::FunPtr(function) => {
                let function = &self.function_types[*function];
                function
                    .parameter_types
                    .iter()
                    .all(|parameter| self.pointee_type_is_valid(*parameter, visiting))
                    && self.pointee_type_is_valid(function.return_type, visiting)
            }
            _ => true,
        };
        visiting.remove(&ty);
        valid
    }

    fn application_pointee_is_valid(
        &self,
        parameters: &[hir::TypeParamDecl],
        requirements: &[hir::RequiresGcFreePointee],
        arguments: &[hir::TypeId],
    ) -> bool {
        requirements.iter().all(|requirement| {
            let index = parameters
                .iter()
                .position(|parameter| parameter.id == requirement.type_param)
                .expect("a nominal pointee requirement names its own parameter");
            self.gc_free_requirements(arguments[index]).is_some()
        })
    }

    fn default_pointee_application_occurrences(&mut self) -> Vec<PointeeApplicationOccurrence> {
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

        let mut out = Vec::new();
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
            let context = match owner {
                crate::defaults::SourceParameterOwner::Function(function) => {
                    RequirementContext::Function(function)
                }
                crate::defaults::SourceParameterOwner::StructConstructor(constructor) => {
                    RequirementContext::Nominal(crate::Owner::Struct(
                        self.struct_constructors[constructor].owner,
                    ))
                }
                crate::defaults::SourceParameterOwner::ClassConstructor(constructor) => {
                    RequirementContext::Nominal(crate::Owner::Class(
                        self.class_constructors[constructor].owner,
                    ))
                }
                crate::defaults::SourceParameterOwner::VariantConstructor(variant) => {
                    RequirementContext::Nominal(crate::Owner::Enum(variant.enumeration()))
                }
            };
            let mut types = body
                .locals
                .iter()
                .map(|(_, local)| local.ty)
                .collect::<Vec<_>>();
            collect_statement_types(self, &body.statements, &mut types);
            collect_expr_types(self, &body.value, &mut types);
            types.push(body.result_type);
            if !bindings.is_empty() {
                types = types
                    .into_iter()
                    .map(|ty| self.instantiate_method_ty(ty, &bindings))
                    .collect();
            }
            out.extend(types.into_iter().map(|ty| PointeeApplicationOccurrence {
                ty,
                file: body.origin.file as usize,
                span: body.origin.span,
                context,
            }));
        }
        out
    }

    fn default_concrete_pointee_application_occurrences(
        &mut self,
    ) -> Vec<PointeeApplicationOccurrence> {
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

        let mut out = Vec::new();
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
            let mut occurrences = Vec::new();
            collect_default_type_occurrences(self, &body, &mut occurrences);
            if !bindings.is_empty() {
                for occurrence in &mut occurrences {
                    occurrence.ty = self.instantiate_method_ty(occurrence.ty, &bindings);
                }
            }
            out.extend(
                occurrences
                    .into_iter()
                    .map(
                        |TypeOccurrence { ty, file, span }| PointeeApplicationOccurrence {
                            ty,
                            file,
                            span,
                            context,
                        },
                    ),
            );
        }
        out
    }

    fn pointee_application_occurrences(&self) -> Vec<PointeeApplicationOccurrence> {
        let mut out = Vec::new();
        for (function, declaration) in self.functions.iter() {
            let file = self
                .function_files
                .get(&function)
                .copied()
                .unwrap_or_else(|| self.primary_output_file());
            for parameter in &declaration.params {
                out.push(PointeeApplicationOccurrence {
                    ty: parameter.ty,
                    file,
                    span: declaration.span,
                    context: RequirementContext::Function(function),
                });
            }
            out.push(PointeeApplicationOccurrence {
                ty: declaration.return_ty,
                file,
                span: declaration.span,
                context: RequirementContext::Function(function),
            });
            if let hir::FunctionKind::User(body) = &declaration.kind {
                let mut occurrences = Vec::new();
                collect_body_type_occurrences(self, body, file, &mut occurrences);
                out.extend(
                    occurrences
                        .into_iter()
                        .map(
                            |TypeOccurrence { ty, file, span }| PointeeApplicationOccurrence {
                                ty,
                                file,
                                span,
                                context: RequirementContext::Function(function),
                            },
                        ),
                );
            }
        }
        for (id, declaration) in self.structs.iter() {
            let file = self.struct_files[&id];
            let context = RequirementContext::Nominal(crate::Owner::Struct(id));
            out.extend(declaration.semantic_fields().iter().map(|field| {
                PointeeApplicationOccurrence {
                    ty: field.ty,
                    file,
                    span: declaration.span,
                    context,
                }
            }));
            out.extend(
                declaration
                    .interfaces
                    .iter()
                    .map(|ty| PointeeApplicationOccurrence {
                        ty: *ty,
                        file,
                        span: declaration.span,
                        context,
                    }),
            );
            for constructor in &declaration.constructors {
                let constructor = &self.struct_constructors[*constructor];
                out.extend(constructor.parameters.iter().map(|parameter| {
                    PointeeApplicationOccurrence {
                        ty: parameter.ty,
                        file,
                        span: constructor.span,
                        context,
                    }
                }));
                let mut occurrences = Vec::new();
                collect_struct_constructor_type_occurrences(self, constructor, &mut occurrences);
                out.extend(
                    occurrences
                        .into_iter()
                        .map(
                            |TypeOccurrence { ty, file, span }| PointeeApplicationOccurrence {
                                ty,
                                file,
                                span,
                                context,
                            },
                        ),
                );
            }
        }
        for (id, declaration) in self.enums.iter() {
            let file = self.enum_files[&id];
            let context = RequirementContext::Nominal(crate::Owner::Enum(id));
            out.extend(declaration.variants.iter().flat_map(|variant| {
                variant
                    .fields
                    .iter()
                    .map(|field| PointeeApplicationOccurrence {
                        ty: field.ty,
                        file,
                        span: declaration.span,
                        context,
                    })
            }));
            out.extend(
                declaration
                    .interfaces
                    .iter()
                    .map(|ty| PointeeApplicationOccurrence {
                        ty: *ty,
                        file,
                        span: declaration.span,
                        context,
                    }),
            );
        }
        for (id, declaration) in self.classes.iter() {
            let file = self.class_files[&id];
            let context = RequirementContext::Nominal(crate::Owner::Class(id));
            out.extend(
                declaration
                    .fields
                    .iter()
                    .map(|field| PointeeApplicationOccurrence {
                        ty: self.class_fields[*field].ty,
                        file,
                        span: declaration.span,
                        context,
                    }),
            );
            out.extend(
                declaration
                    .base_class
                    .iter()
                    .chain(&declaration.interfaces)
                    .map(|ty| PointeeApplicationOccurrence {
                        ty: *ty,
                        file,
                        span: declaration.span,
                        context,
                    }),
            );
            for constructor in &declaration.constructors {
                let constructor = &self.class_constructors[*constructor];
                out.extend(constructor.parameters.iter().map(|parameter| {
                    PointeeApplicationOccurrence {
                        ty: parameter.ty,
                        file,
                        span: constructor.span,
                        context,
                    }
                }));
                let mut occurrences = Vec::new();
                collect_class_constructor_type_occurrences(self, constructor, &mut occurrences);
                out.extend(
                    occurrences
                        .into_iter()
                        .map(
                            |TypeOccurrence { ty, file, span }| PointeeApplicationOccurrence {
                                ty,
                                file,
                                span,
                                context,
                            },
                        ),
                );
            }
        }
        for (id, declaration) in self.interfaces.iter() {
            let file = self.interface_files[&id];
            let context = RequirementContext::Nominal(crate::Owner::Interface(id));
            out.extend(
                declaration
                    .parents
                    .iter()
                    .map(|parent| PointeeApplicationOccurrence {
                        ty: *parent,
                        file,
                        span: declaration.span,
                        context,
                    }),
            );
        }
        out.extend(
            self.type_aliases
                .iter()
                .map(|(_, declaration)| PointeeApplicationOccurrence {
                    ty: declaration.target,
                    file: declaration.origin.file as usize,
                    span: declaration.origin.span,
                    context: RequirementContext::Closed,
                }),
        );
        out.extend(self.globals.iter().map(|(_, global)| {
            PointeeApplicationOccurrence {
                ty: global.ty,
                file: self
                    .property_files
                    .get(&global.property)
                    .copied()
                    .unwrap_or_else(|| self.primary_output_file()),
                span: global.span,
                context: RequirementContext::Closed,
            }
        }));
        out
    }
}
