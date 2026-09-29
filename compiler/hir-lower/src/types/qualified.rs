//! Package selection and static owner traversal for source type paths.

use super::*;
use crate::NominalTarget;
use crate::imports::lookup::TypeLookupTarget;
use crate::namespace::TopLevelTypeTarget;

pub(crate) enum TypeQualifier {
    Current(NominalTarget),
    Imported(hir::SourceNominalId),
}

impl Lowerer {
    pub(super) fn resolve_qualified_nominal(
        &mut self,
        path: &[ast::Ident],
        arguments: &[ast::TypeRef],
        span: ast::Span,
    ) -> Option<TypeId> {
        let first = path.first().expect("a qualified type path is non-empty");
        let (binding, start) = if let Some(package) = self.qualified_package_prefix(path) {
            let length = package.consumed();
            let package_name = package.name();
            let Some(name) = path.get(length) else {
                self.error(
                    path.last().expect("the path is non-empty").span,
                    format!("`{package_name}` names a package, not a type"),
                );
                return None;
            };
            let lookup = self.lookup_package_type(&package, &name.text);
            let Some(binding) = self.commit_type_lookup(name, lookup).ok()? else {
                self.error(
                    name.span,
                    format!(
                        "package `{package_name}` has no accessible type `{}`",
                        name.text
                    ),
                );
                return None;
            };
            (binding, length)
        } else if let Some(target) = self.lexical_nested_nominal_target(&first.text) {
            (
                TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target)),
                0,
            )
        } else {
            let Some(binding) = self.resolve_type_lookup(first).ok()? else {
                self.error(first.span, format!("unknown type `{}`", first.text));
                return None;
            };
            (binding, 0)
        };
        let name = &path[start];
        if start + 1 == path.len() {
            return self.resolve_qualified_type_binding(binding, name, arguments, span);
        }
        let mut target = match self.resolve_namespace_type_binding(binding, name)? {
            TypeQualifier::Imported(owner) => {
                return self.resolve_imported_qualified_type(owner, &path[start + 1..], arguments);
            }
            TypeQualifier::Current(target) => target,
        };
        for segment in &path[start + 1..] {
            let Some(nested) = self
                .nested_nominals_by_owner
                .get(&(target.owner(), segment.text.clone()))
                .copied()
            else {
                let owner_name = path
                    .iter()
                    .take_while(|candidate| candidate.span.end <= segment.span.start)
                    .map(|candidate| candidate.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.error(
                    segment.span,
                    format!("type `{owner_name}` has no nested type `{}`", segment.text),
                );
                return None;
            };
            target = nested;
        }
        let display = path
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        self.resolve_nested_nominal_application(target, arguments, span, &display)
    }

    fn resolve_qualified_type_binding(
        &mut self,
        binding: TypeLookupTarget,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
        span: ast::Span,
    ) -> Option<TypeId> {
        match binding {
            TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target)) => {
                self.resolve_nested_nominal_application(target, arguments, span, &name.text)
            }
            TypeLookupTarget::Current(TopLevelTypeTarget::Alias(alias)) => {
                self.resolve_type_alias_id_reference(alias, name, !arguments.is_empty())
            }
            TypeLookupTarget::Dependency(binding) => {
                if arguments.is_empty() {
                    self.resolve_imported_dependency_type_target(&binding, name, false)
                } else {
                    self.resolve_imported_generic_type_target(&binding, name, arguments)
                }
            }
        }
    }

    pub(crate) fn resolve_namespace_type_binding(
        &mut self,
        binding: TypeLookupTarget,
        name: &ast::Ident,
    ) -> Option<TypeQualifier> {
        if let TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target)) = &binding {
            return Some(TypeQualifier::Current(*target));
        }
        if let TypeLookupTarget::Dependency(binding) = &binding
            && let Some(owner) = binding.target().source_nominal()
        {
            return Some(TypeQualifier::Imported(owner));
        }
        let ty = self.resolve_qualified_type_binding(binding, name, &[], name.span)?;
        if let Some(target) = self.nominal_target_for_type(ty) {
            return Some(TypeQualifier::Current(target));
        }
        if let Some(owner) = self.imported_nominal_owner(ty) {
            return Some(TypeQualifier::Imported(owner));
        }
        self.error(
            name.span,
            format!("typealias `{}` does not name a type qualifier", name.text),
        );
        None
    }
}
