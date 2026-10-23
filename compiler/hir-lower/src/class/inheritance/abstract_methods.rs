//! A concrete class must provide every inherited virtual implementation.

use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn check_abstract_class_methods(
        &mut self,
        class: ClassId,
        span: ast::Span,
        host: &str,
    ) {
        if self.classes[class].modifier == hir::ClassModifier::Abstract {
            return;
        }
        let application = self.classes[class].self_application;
        let mut current = Some((
            self.class_applications[application].canonical_type,
            Type::Class(application),
        ));
        let mut seen_types = std::collections::HashSet::new();
        let mut seen_families = std::collections::HashSet::new();
        while let Some((ty, class_type)) = current {
            if !seen_types.insert(ty) {
                break;
            }
            match class_type {
                Type::Class(application)
                    if self
                        .source_class_id(self.class_applications[application].template)
                        .is_none() =>
                {
                    let application = self.class_applications[application].clone();
                    let class = self.loaded_class_definitions[&application.template].clone();
                    for method in &class.virtual_methods {
                        let selection = class
                            .declaration
                            .interface
                            .declaration_details()
                            .dispatch_selections()
                            .get(
                                &hir::NominalDispatchSelectionRoleV1::ClassVtable,
                                method.slot,
                            )
                            .expect("dependency virtual family has a complete selection");
                        let declaration = self
                            .dependencies
                            .as_ref()
                            .expect("dependency class has a catalog")
                            .callable_declaration(selection.callable_target())
                            .expect("dependency slot has an actual declaration");
                        if let Some(condition) =
                            self.imported_member_condition(declaration.interface(), ty)
                            && self.conditional_application_is_impossible(&condition)
                        {
                            continue;
                        }
                        if !seen_families.insert(method.family) {
                            continue;
                        }
                        if matches!(
                            selection.selection(),
                            hir::InheritanceSourceSlotSelectionV1::Abstract(_)
                        ) {
                            let kind = match selection.selection().declaration() {
                                hir::InheritanceCallableDeclarationV1::Function(_) => "method",
                                hir::InheritanceCallableDeclarationV1::Getter(_) => "getter",
                                hir::InheritanceCallableDeclarationV1::Setter(_) => "setter",
                            };
                            self.error(
                                span,
                                format!(
                                    "{host} does not implement abstract {kind} `{}.{}`",
                                    class.declaration.name(),
                                    declaration.name()
                                ),
                            );
                        }
                    }
                    break;
                }
                Type::Class(application) => {
                    let application = self.class_applications[application].clone();
                    let inherited = application.template
                        != self
                            .nominal_identity(crate::Owner::Class(class))
                            .declaration_id();
                    let class = self.classes[self.class_id(application.template)].clone();
                    current = class.base_class.map(|base| {
                        let base = self.instantiate_ty(base, &application.arguments);
                        (base, self.types[base].clone())
                    });
                    for function in class.methods {
                        let method = self.functions[function]
                            .method
                            .expect("class member has method metadata");
                        if let Some(condition) =
                            self.local_member_condition(function, &application.arguments)
                            && self.conditional_application_is_impossible(&condition)
                        {
                            continue;
                        }
                        let family = match method.dispatch {
                            hir::MethodDispatch::Virtual(family)
                            | hir::MethodDispatch::FinalOverride(family) => family,
                            _ => continue,
                        };
                        if seen_families.insert(family)
                            && inherited
                            && method.modifier == hir::MethodModifier::Abstract
                        {
                            self.error(
                                span,
                                format!(
                                    "{host} does not implement abstract method `{}`",
                                    self.functions[function].name
                                ),
                            );
                        }
                    }
                }
                _ => unreachable!("class inheritance follows class types"),
            }
        }
    }
}
