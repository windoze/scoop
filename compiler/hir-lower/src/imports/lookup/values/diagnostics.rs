use super::{NonValueTarget, ResolvedValueTarget, ValueOrigin, ValueTarget};
use crate::{
    Lowerer,
    imports::{ImportLookupLayer, lookup::LookupResult},
    namespace::TopLevelTypeTarget,
};
use scoop_ast as ast;
use scoop_hir as hir;

impl Lowerer {
    fn value_origin_location(&self, origin: &ValueOrigin) -> (usize, ast::Span) {
        match origin {
            ValueOrigin::CoreNonValue(NonValueTarget::Function(id)) => {
                (self.function_files[id], self.functions[*id].span)
            }
            ValueOrigin::CoreNonValue(NonValueTarget::ImportedCoreCallable(_)) => {
                (self.current_file, ast::Span::new(0, 0))
            }
            ValueOrigin::CoreNonValue(NonValueTarget::ImportedDependency(_)) => {
                unreachable!("core blockers cannot carry ordinary dependency targets")
            }
            ValueOrigin::CoreNonValue(NonValueTarget::Type(target)) => self
                .type_candidate_location(&super::super::TypeLookupCandidate {
                    target: super::super::TypeLookupTarget::Current(*target),
                    origin: super::super::TypeLookupOrigin::ExistingM22Core,
                }),
            ValueOrigin::CoreNonValue(NonValueTarget::ExtensionProperty(id)) => {
                (self.property_files[id], self.properties[*id].span)
            }
            ValueOrigin::CoreNonValue(NonValueTarget::SourceExtensionProperty(_)) => {
                unreachable!("core has no provisional import properties")
            }
            ValueOrigin::CurrentUnit(id) | ValueOrigin::NonValue { binding: id, .. } => {
                let binding = self.imports.binding(*id);
                (binding.file, binding.span)
            }
            ValueOrigin::Core(ValueTarget::Property(id)) => {
                (self.property_files[id], self.properties[*id].span)
            }
            ValueOrigin::Core(ValueTarget::Object(id)) => {
                (self.object_files[id], self.objects[*id].span)
            }
            ValueOrigin::Core(ValueTarget::Variant(target)) => (
                self.enum_files[&target.enumeration()],
                self.enums[target.enumeration()].span,
            ),
            ValueOrigin::RejectedFunction(id) => {
                (self.function_files[id], self.functions[*id].span)
            }
            ValueOrigin::Dependency(_) => (self.current_file, ast::Span::new(0, 0)),
        }
    }

    pub(crate) fn resolve_value_name(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<ResolvedValueTarget>, ()> {
        self.resolve_value_origin(name).map(|origin| {
            origin.map(|origin| match origin {
                ValueOrigin::Dependency(binding) => ResolvedValueTarget::Dependency(binding),
                origin => ResolvedValueTarget::Materialized(
                    self.materialized_value_target(&origin)
                        .expect("body lookup follows complete declaration materialization"),
                ),
            })
        })
    }

    pub(crate) fn resolve_value_origin(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<ValueOrigin>, ()> {
        let (message, candidates) = match self.lookup_value_origin(&name.text) {
            LookupResult::Missing => return Ok(None),
            LookupResult::Unique(ValueOrigin::RejectedFunction(_)) => return Err(()),
            LookupResult::Unique(origin) => match Self::non_value_origin(&origin) {
                Some(target) => (
                    Self::non_value_message(name, target),
                    ast::NonEmptyVec::new(origin, Vec::new()),
                ),
                None => return Ok(Some(origin)),
            },
            LookupResult::Ambiguous { layer, candidates } => {
                let layer = match layer {
                    ImportLookupLayer::Exact => "exact import",
                    ImportLookupLayer::CurrentPackage(_) => "current package",
                    ImportLookupLayer::Star => "star import",
                    ImportLookupLayer::CorePrelude => "core prelude",
                };
                let message = if candidates.iter().all(|origin| {
                    matches!(
                        Self::non_value_origin(origin),
                        Some(
                            NonValueTarget::Function(_)
                                | NonValueTarget::ImportedDependency(
                                    hir::ImportedTarget::Function(_)
                                        | hir::ImportedTarget::GenericFunction(_)
                                )
                        )
                    )
                }) {
                    format!(
                        "function `{}` is not a value; use `::{}` to create a callable reference",
                        name.text, name.text
                    )
                } else {
                    format!("value `{}` is ambiguous in the {layer} layer", name.text)
                };
                (message, candidates)
            }
            LookupResult::Inaccessible(candidates) => (
                format!("value `{}` is not accessible here", name.text),
                candidates,
            ),
        };
        self.push_value_origin_diagnostic(name, message, candidates.as_slice());
        Err(())
    }

    pub(crate) fn diagnose_value_layer(
        &mut self,
        name: &ast::Ident,
        layer: ImportLookupLayer,
        candidates: &[ValueOrigin],
    ) {
        assert!(
            !candidates.is_empty(),
            "a failed value layer has candidates"
        );
        let message = if candidates.len() == 1 {
            let target = Self::non_value_origin(&candidates[0])
                .expect("one selected value candidate is not an ambiguity");
            Self::non_value_message(name, target)
        } else if candidates.iter().all(|origin| {
            matches!(
                Self::non_value_origin(origin),
                Some(
                    NonValueTarget::Function(_)
                        | NonValueTarget::ImportedDependency(
                            hir::ImportedTarget::Function(_)
                                | hir::ImportedTarget::GenericFunction(_)
                        )
                )
            )
        }) {
            format!(
                "function `{}` is not a value; use `::{}` to create a callable reference",
                name.text, name.text
            )
        } else {
            let layer = match layer {
                ImportLookupLayer::Exact => "exact import",
                ImportLookupLayer::CurrentPackage(_) => "current package",
                ImportLookupLayer::Star => "star import",
                ImportLookupLayer::CorePrelude => "core prelude",
            };
            format!("value `{}` is ambiguous in the {layer} layer", name.text)
        };
        self.push_value_origin_diagnostic(name, message, candidates);
    }

    fn push_value_origin_diagnostic(
        &mut self,
        name: &ast::Ident,
        message: String,
        candidates: &[ValueOrigin],
    ) {
        let mut diagnostic = ast::Diagnostic::at_file(self.current_file, name.span, message);
        let mut locations = candidates
            .iter()
            .map(|origin| self.value_origin_location(origin))
            .collect::<Vec<_>>();
        locations.sort_by_key(|(file, span)| (*file, span.start, span.end));
        for (file, span) in locations {
            diagnostic.notes.push(ast::DiagnosticNote {
                file,
                span,
                message: "candidate declared here".to_string(),
            });
        }
        self.diagnostics.push(diagnostic);
    }

    fn non_value_message(name: &ast::Ident, target: NonValueTarget) -> String {
        match target {
            NonValueTarget::Function(_)
            | NonValueTarget::ImportedCoreCallable(_)
            | NonValueTarget::ImportedDependency(
                hir::ImportedTarget::Function(_) | hir::ImportedTarget::GenericFunction(_),
            ) => format!(
                "function `{}` is not a value; use `::{}` to create a callable reference",
                name.text, name.text
            ),
            NonValueTarget::ImportedDependency(hir::ImportedTarget::Type(_))
            | NonValueTarget::ImportedDependency(hir::ImportedTarget::GenericType(_)) => {
                format!("type `{}` is a type, not a value", name.text)
            }
            NonValueTarget::ImportedDependency(hir::ImportedTarget::TypeAlias(_)) => {
                format!("typealias `{}` is a type, not a value", name.text)
            }
            NonValueTarget::ImportedDependency(hir::ImportedTarget::ExtensionProperty(_)) => {
                format!("extension property `{}` requires a receiver", name.text)
            }
            NonValueTarget::ImportedDependency(
                hir::ImportedTarget::ObjectValue(_)
                | hir::ImportedTarget::Property(_)
                | hir::ImportedTarget::EnumVariant(_),
            ) => format!(
                "dependency value `{}` requires a later cross-Cone capability",
                name.text
            ),
            NonValueTarget::Type(TopLevelTypeTarget::Alias(_)) => {
                format!("typealias `{}` is a type, not a value", name.text)
            }
            NonValueTarget::Type(TopLevelTypeTarget::Nominal(_)) => {
                format!("type `{}` is a type, not a value", name.text)
            }
            NonValueTarget::ExtensionProperty(_) | NonValueTarget::SourceExtensionProperty(_) => {
                format!("extension property `{}` requires a receiver", name.text)
            }
        }
    }
}
