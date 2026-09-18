use super::{NamedPropertyReceiver, ValueOrigin, ValueTarget};
use crate::{Lowerer, Owner, imports::CurrentUnitTarget};
use scoop_ast as ast;
use scoop_hir as hir;

impl Lowerer {
    /// Early static preflight may return None for an unallocated declaration;
    /// it must then decline folding, not select a different origin.
    pub(crate) fn materialized_value_target(&self, origin: ValueOrigin) -> Option<ValueTarget> {
        Some(match origin {
            ValueOrigin::NonValue { .. }
            | ValueOrigin::CoreNonValue(_)
            | ValueOrigin::DependencyNonValue(_)
            | ValueOrigin::RejectedFunction(_) => return None,
            ValueOrigin::Core(target) => target,
            ValueOrigin::CurrentUnit(id) => match self.imports.binding(id).target {
                CurrentUnitTarget::Property(id) => ValueTarget::Property(id),
                CurrentUnitTarget::Object(id) => ValueTarget::Object(id),
                CurrentUnitTarget::EnumVariant(target) => ValueTarget::Variant(target),
                CurrentUnitTarget::SourceProperty(id) => {
                    ValueTarget::Property(*self.imports.resolved_properties.get(&id)?)
                }
                CurrentUnitTarget::SourceVariant(id) => {
                    ValueTarget::Variant(*self.imports.resolved_variants.get(&id)?)
                }
                _ => unreachable!("value origin was selected from the value surface"),
            },
        })
    }

    pub(crate) fn named_property_receiver(
        &mut self,
        property: hir::PropertyId,
        span: ast::Span,
    ) -> Option<NamedPropertyReceiver> {
        if matches!(
            self.properties[property].representation,
            hir::PropertyRepresentation::Const { .. }
        ) {
            return Some(NamedPropertyReceiver::None);
        }
        Some(match self.properties[property].owner {
            hir::PropertyOwner::TopLevel => NamedPropertyReceiver::None,
            hir::PropertyOwner::Object(object) => NamedPropertyReceiver::Singleton {
                value: self.lower_singleton_value(object, span)?,
                owner: self.method_owner_application(Owner::Object(object), Vec::new()),
            },
            _ => unreachable!("bare property origin is top-level or singleton-owned"),
        })
    }
}
