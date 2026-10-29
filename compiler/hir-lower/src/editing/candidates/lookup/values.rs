use super::*;
use crate::editing::candidates::properties::RankedValue;
use crate::imports::lookup::calls::ExtensionPropertyTarget;
use crate::imports::lookup::values::{ResolvedValueTarget, ValueTarget};

impl Lowerer {
    pub(super) fn editing_named_values(
        &self,
        bindings: &[NamedCallBinding],
        name: &ast::Ident,
        rank: crate::imports::lookup::calls::ImportLookupRank,
    ) -> Result<Option<RankedValue>, U> {
        let mut values =
            bindings
                .iter()
                .filter_map(|binding| match (&binding.target, &binding.origin) {
                    (NamedCallTarget::Value(ValueTarget::Variant(_)), _) => None,
                    (NamedCallTarget::Value(value), _) => {
                        Some(ResolvedValueTarget::Materialized(*value))
                    }
                    (
                        NamedCallTarget::ImportedDependency(
                            hir::ImportedTarget::Property(_) | hir::ImportedTarget::ObjectValue(_),
                        ),
                        NamedCallOrigin::Dependency(binding),
                    ) => Some(ResolvedValueTarget::Dependency(binding.clone())),
                    _ => None,
                });
        let value = values.next();
        if values.next().is_some() {
            return Err(U::DeclarationFailure);
        }
        let mut read = match value {
            Some(value) => Some(
                self.probe_expr_layer(|state, _| {
                    state.lower_resolved_value_target(name, value, None)
                })
                .map_err(|_| U::BodyFailure)?,
            ),
            None => None,
        };
        let extension_properties = bindings
            .iter()
            .filter_map(|binding| match (binding.target, &binding.origin) {
                (NamedCallTarget::ExtensionProperty(id), _) => {
                    Some(ExtensionPropertyTarget::Current(id))
                }
                (
                    NamedCallTarget::ImportedDependency(hir::ImportedTarget::ExtensionProperty(_)),
                    NamedCallOrigin::Dependency(binding),
                ) => Some(ExtensionPropertyTarget::Dependency(binding.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !extension_properties.is_empty() {
            for binding in self.implicit_receivers() {
                let mut state = self.clone();
                let receiver = state
                    .lower_receiver_binding(binding, name.span)
                    .ok_or(U::BodyFailure)?;
                if let Some(property) =
                    state.editing_extension_property_read(receiver, name, &extension_properties)?
                {
                    if read.is_some() {
                        return Err(U::DeclarationFailure);
                    }
                    read = Some(property);
                    break;
                }
            }
        }
        Ok(read.map(|view| RankedValue { view, rank }))
    }
}
