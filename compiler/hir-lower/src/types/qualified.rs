//! Package selection and static owner traversal for source type paths.

use super::*;
use crate::NominalTarget;
use crate::imports::lookup::TypeLookupTarget;

mod names;
pub(crate) use names::ResolvedTypeName;

impl Lowerer {
    pub(super) fn resolve_qualified_nominal(
        &mut self,
        path: &[ast::Ident],
        arguments: &[ast::TypeRef],
        span: ast::Span,
    ) -> Option<TypeId> {
        let Some(target) = self
            .resolve_type_name_path(path, !arguments.is_empty())
            .ok()?
        else {
            let first = path.first().expect("a qualified type path is non-empty");
            self.error(first.span, format!("unknown type `{}`", first.text));
            return None;
        };
        match target {
            ResolvedTypeName::Alias(ty) => Some(ty),
            ResolvedTypeName::Nominal(owner) => {
                if let Some(target) = self.current_nominal_name_target(owner) {
                    let display = path
                        .iter()
                        .map(|name| name.text.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    self.resolve_nested_nominal_application(target, arguments, span, &display)
                } else {
                    let name = path.last().expect("a type path has a final name");
                    self.resolve_imported_nominal_owner_arguments(owner, name, arguments)
                }
            }
        }
    }
}
