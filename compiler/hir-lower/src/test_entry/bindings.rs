use super::*;
use crate::imports::CurrentUnitTarget;
use crate::imports::lookup::calls::{NamedCallBinding, NamedCallOrigin, NamedCallTarget};
use crate::namespace::{TopLevelLookupLayer, TopLevelTypeTarget};

pub(super) fn test_cone() -> scoop_identity::ConeIdentity {
    scoop_identity::ConeCoordinate::new("scoop", "scoop.test", "0.1.0")
        .unwrap()
        .identity()
        .unwrap()
}

impl Lowerer {
    pub(super) fn test_api_binding(
        &mut self,
        name: &str,
        span: ast::Span,
    ) -> Option<NamedCallBinding> {
        let binding = if self.current_cone() == test_cone() {
            let package = (0..self.intrinsic_sources.len()).find_map(|file| {
                let TopLevelLookupLayer::CurrentPackage(package) =
                    self.top_level_namespaces.source_namespace(file)
                else {
                    return None;
                };
                (self.top_level_namespaces.package_segments(package)
                    == ["scoop", "test", "execution"])
                .then_some(package)
            });
            package.and_then(|package| {
                let bindings = self.imports.package_bindings(package, name);
                let [binding] = bindings.as_slice() else {
                    return None;
                };
                let target = match self.imports.binding(*binding).target {
                    CurrentUnitTarget::Function(function) => NamedCallTarget::Function(function),
                    CurrentUnitTarget::Struct(structure) => NamedCallTarget::Type(
                        TopLevelTypeTarget::Nominal(crate::NominalTarget::Struct(structure)),
                    ),
                    CurrentUnitTarget::Enum(enumeration) => NamedCallTarget::Type(
                        TopLevelTypeTarget::Nominal(crate::NominalTarget::Enum(enumeration)),
                    ),
                    _ => return None,
                };
                Some(NamedCallBinding {
                    target,
                    origin: NamedCallOrigin::CurrentUnit(*binding),
                })
            })
        } else {
            self.test_compilation
                .as_ref()
                .unwrap()
                .bindings
                .iter()
                .find(|binding| binding.name().as_str() == name)
                .and_then(|binding| match binding.targets() {
                    [target] => Some(NamedCallBinding {
                        target: NamedCallTarget::ImportedDependency(target.target()),
                        origin: NamedCallOrigin::Dependency(target.clone()),
                    }),
                    _ => None,
                })
        };
        if binding.is_none() {
            self.error(span, format!("test execution requires the ordinary `scoop.test.execution.{name}` declaration from `scoop:scoop.test:0.1.0`"));
        }
        binding
    }

    pub(super) fn test_api_type(&mut self, name: &str, span: ast::Span) -> Option<hir::TypeId> {
        let binding = self.test_api_binding(name, span)?;
        match binding.target {
            NamedCallTarget::Type(TopLevelTypeTarget::Nominal(target)) => {
                Some(self.owner_ty(target.owner()))
            }
            NamedCallTarget::ImportedDependency(_) => {
                let NamedCallOrigin::Dependency(binding) = binding.origin else {
                    unreachable!("dependency target has its binding")
                };
                self.resolve_imported_dependency_type_target(
                    &binding,
                    &ast::Ident {
                        text: name.into(),
                        span,
                    },
                    false,
                )
            }
            _ => {
                self.error(
                    span,
                    format!("test execution requires `{name}` to be a type"),
                );
                None
            }
        }
    }
}
