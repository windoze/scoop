//! Checks the official test markers after ordinary annotation binding.

use scoop_ast::{Diagnostic, DiagnosticNote};
use scoop_hir as hir;
use std::collections::HashMap;

use crate::{Lowerer, Owner};

mod markers;
use markers::TestMarker;

impl Lowerer {
    pub(crate) fn validate_test_annotation_source(
        &mut self,
        application: &hir::SourceAnnotationApplication,
    ) {
        if let Some(marker) = TestMarker::from_identity(application.annotation)
            && self.intrinsic_sources[application.definition_origin.file as usize].role
                != scoop_identity::SourceRole::Test
        {
            self.test_annotation_error(
                application.definition_origin,
                format!("`@{}` is only allowed in test sources", marker.name(),),
            );
        }
    }

    pub(crate) fn validate_test_declarations(&mut self) {
        let mut targets = self
            .annotation_metadata
            .targets
            .iter()
            .filter_map(|target| {
                let markers = target
                    .annotations
                    .iter()
                    .filter_map(|application| {
                        TestMarker::from_identity(application.annotation)
                            .map(|marker| (marker, application.definition_origin))
                    })
                    .collect::<Vec<_>>();
                (!markers.is_empty()).then_some((target.target, markers))
            })
            .collect::<Vec<_>>();
        targets.sort_by_key(|(_, markers)| (markers[0].1.file, markers[0].1.span.start));
        let mut containers = HashMap::new();
        for (target, markers) in &targets {
            let hir::SourceAnnotationTarget::Nominal(owner) = *target else {
                continue;
            };
            let Some((_, origin)) = markers
                .iter()
                .find(|(marker, _)| *marker == TestMarker::Test)
            else {
                continue;
            };
            match owner {
                hir::NominalOwner::Class(class) if self.valid_test_container(class) => {
                    containers.insert(class, *origin);
                }
                _ => self.test_annotation_error(*origin,
                    "a test container must be a top-level non-generic concrete class; abstract, sealed, local and nested declarations are not allowed".into()),
            }
        }
        let methods = self
            .classes
            .iter()
            .flat_map(|(class, declaration)| {
                declaration
                    .methods
                    .iter()
                    .map(move |&method| (method, class))
            })
            .collect::<HashMap<_, _>>();
        let mut hooks = HashMap::new();
        for (target, markers) in targets {
            let hir::SourceAnnotationTarget::Callable(hir::SourceAnnotationCallable::Function(
                function,
            )) = target
            else {
                continue;
            };
            let origin = markers[0].1;
            let roles = markers
                .iter()
                .filter(|(marker, _)| *marker != TestMarker::Ignore)
                .collect::<Vec<_>>();
            if markers
                .iter()
                .any(|(marker, _)| *marker == TestMarker::Ignore)
                && !markers
                    .iter()
                    .any(|(marker, _)| *marker == TestMarker::Test)
            {
                self.test_annotation_error(
                    origin,
                    "`@Ignore` requires `@Test` on the same test method".into(),
                );
            }
            if roles.len() > 1 {
                self.test_annotation_error(
                    origin,
                    "a method may have only one of `@Test`, `@BeforeTest` and `@AfterTest`".into(),
                );
            }
            let Some(&class) = methods
                .get(&function)
                .filter(|class| containers.contains_key(class))
            else {
                self.test_annotation_error(
                    origin,
                    "a test or lifecycle method must be declared directly in a `@Test` class"
                        .into(),
                );
                continue;
            };
            if let Some(reason) = self.test_method_rejection(function) {
                self.test_annotation_error(origin, format!("invalid test or lifecycle method: {reason}; expected a safe synchronous instance method with no parameters or type parameters returning Unit"));
            }
            for &(marker, origin) in roles {
                if matches!(marker, TestMarker::Before | TestMarker::After)
                    && let Some(previous) = hooks.insert((class, marker), origin)
                {
                    self.diagnostics.push(
                        Diagnostic::at_file(
                            origin.file as usize,
                            origin.span,
                            format!(
                                "a test container may declare at most one `@{}` method",
                                marker.name()
                            ),
                        )
                        .with_note(DiagnosticNote::at(
                            previous.file as usize,
                            previous.span,
                            "the other lifecycle method is declared here",
                        )),
                    );
                }
            }
        }
    }

    fn valid_test_container(&self, class: hir::ClassId) -> bool {
        let declaration = &self.classes[class];
        let Some(identity) = self.nominal_identity(Owner::Class(class)).source() else {
            return false;
        };
        let identity = identity.declaration();
        declaration.owner.is_none()
            && !matches!(
                identity.scope(),
                scoop_identity::DeclarationScope::LexicalScoped { .. }
            )
            && declaration.type_params.is_empty()
            && declaration.modifier != hir::ClassModifier::Abstract
            && matches!(declaration.inheritance_family, hir::InheritanceFamily::Open)
            && declaration.is_declared()
    }

    fn test_method_rejection(&self, function: hir::FunctionId) -> Option<&'static str> {
        let declaration = &self.functions[function];
        if declaration.method.is_none() {
            return Some("missing instance receiver");
        }
        if self.extension_receivers.contains_key(&function) {
            return Some("extension receiver is not allowed");
        }
        if !declaration.context_parameters.is_empty() {
            return Some("context parameters are not allowed");
        }
        if declaration.params.len() != 1 {
            return Some("value parameters are not allowed, including defaulted parameters");
        }
        if !matches!(declaration.genericity, hir::FunctionGenericity::Plain) {
            return Some("type parameters are not allowed");
        }
        if declaration.is_suspend {
            return Some("suspend is not allowed");
        }
        if declaration.return_ty != self.unit {
            return Some("return type must be Unit");
        }
        if declaration.attributes.safety != hir::Safety::Safe {
            return Some("unsafe calls are not allowed");
        }
        if !matches!(declaration.kind, hir::FunctionKind::User(_)) {
            return Some("a Scoop function body is required");
        }
        None
    }

    fn test_annotation_error(&mut self, origin: hir::DefinitionOrigin, message: String) {
        self.diagnostics.push(Diagnostic::at_file(
            origin.file as usize,
            origin.span,
            message,
        ));
    }
}
