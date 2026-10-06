use super::{NonValueTarget, ResolvedValueTarget, ValueOrigin, ValueTarget};
use crate::{
    Lowerer,
    imports::{ImportLookupLayer, lookup::LookupResult},
    namespace::TopLevelTypeTarget,
};
use scoop_ast as ast;
use scoop_hir as hir;

impl Lowerer {
    fn value_origin_note(&self, origin: &ValueOrigin) -> Result<ast::DiagnosticNote, String> {
        let (file, span) = match origin {
            ValueOrigin::CoreNonValue(NonValueTarget::Function(id)) => {
                (self.function_files[id], self.functions[*id].span)
            }
            ValueOrigin::CoreNonValue(NonValueTarget::ImportedDependency(_)) => {
                unreachable!("core blockers cannot carry ordinary dependency targets")
            }
            ValueOrigin::CoreNonValue(NonValueTarget::Type(target)) => {
                return self.type_candidate_note(&super::super::TypeLookupCandidate {
                    target: super::super::TypeLookupTarget::Current(*target),
                    origin: super::super::TypeLookupOrigin::ExistingM22Core,
                });
            }
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
            ValueOrigin::Dependency(binding) => {
                return self.dependency_candidate_note(binding.target());
            }
        };
        Ok(ast::DiagnosticNote::at(
            file,
            span,
            "candidate declared here",
        ))
    }

    pub(crate) fn resolve_value_name(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<ResolvedValueTarget>, ()> {
        self.resolve_value_name_with_layer(name)
            .map(|resolved| resolved.map(|(target, _)| target))
    }

    pub(crate) fn resolve_value_name_with_layer(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<(ResolvedValueTarget, ImportLookupLayer)>, ()> {
        let (layer, lookup) = self.lookup_value_origin_with_layer(&name.text);
        self.resolve_value_lookup(name, lookup).map(|origin| {
            origin.map(|origin| {
                let target = match origin {
                    ValueOrigin::Dependency(binding) => ResolvedValueTarget::Dependency(binding),
                    origin => ResolvedValueTarget::Materialized(
                        self.materialized_value_target(&origin)
                            .expect("body lookup follows complete declaration materialization"),
                    ),
                };
                (
                    target,
                    layer.expect("a resolved value belongs to a lookup layer"),
                )
            })
        })
    }

    pub(crate) fn resolve_value_origin(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<ValueOrigin>, ()> {
        self.resolve_value_lookup(name, self.lookup_value_origin(&name.text))
    }

    fn resolve_value_lookup(
        &mut self,
        name: &ast::Ident,
        lookup: LookupResult<ValueOrigin>,
    ) -> Result<Option<ValueOrigin>, ()> {
        let (message, candidates) = match lookup {
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
                    ImportLookupLayer::QualifiedPackage => "qualified package",
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
                ImportLookupLayer::QualifiedPackage => "qualified package",
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
        match candidates
            .iter()
            .map(|origin| self.value_origin_note(origin))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(mut notes) => {
                super::super::diagnostics::sort_candidate_notes(&mut notes);
                diagnostic.notes = notes;
            }
            Err(message) => diagnostic.message = message,
        }
        self.diagnostics.push(diagnostic);
    }

    fn non_value_message(name: &ast::Ident, target: NonValueTarget) -> String {
        match target {
            NonValueTarget::Function(_)
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
            NonValueTarget::ImportedDependency(hir::ImportedTarget::Annotation(_)) => {
                format!(
                    "annotation `{}` is only usable as a static annotation",
                    name.text
                )
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
            NonValueTarget::Type(TopLevelTypeTarget::Annotation(_)) => format!(
                "annotation `{}` is only usable as a static annotation",
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
