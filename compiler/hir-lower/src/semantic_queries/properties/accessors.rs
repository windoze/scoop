use super::*;

impl Lowerer {
    pub(super) fn query_current_getter(
        &mut self,
        receiver: hir::TypeId,
        property: hir::PropertyId,
        getter: hir::PropertyGetterId,
        file: usize,
    ) -> hir::SemanticAccessorDescription {
        let owner = self.ordinary_property_identity(property).property_owner();
        let identity = self
            .getter_identity_records
            .entry(getter)
            .or_insert_with(|| {
                hir::HirPropertyAccessorIdentity::getter(property, owner)
                    .expect("a checked property identifies its getter")
            })
            .id();
        let value = &self.property_getters[getter];
        hir::SemanticAccessorDescription {
            identity,
            callable: hir::SemanticCallable::CurrentGetter(getter),
            access: hir::SemanticMemberAccess {
                visibility: value.access.declared.into(),
                domain: value.access.lookup.0.clone(),
                accessible: self.property_accessor_is_accessible(
                    property,
                    &value.access,
                    Some(receiver),
                ),
            },
            implementation: hir::PropertyAccessorImplementationV1::from_source(
                value.implementation,
            ),
            annotations: hir::SemanticAnnotationTarget::Current(
                hir::SourceAnnotationTarget::Callable(hir::SourceAnnotationCallable::Getter(
                    getter,
                )),
            ),
            origin: hir::SemanticSourceLocation {
                source: self.source_identity(file).clone(),
                span: value.span,
            },
        }
    }

    pub(super) fn query_current_setter(
        &mut self,
        receiver: hir::TypeId,
        property: hir::PropertyId,
        setter: hir::PropertySetterId,
        file: usize,
    ) -> hir::SemanticAccessorDescription {
        let owner = self.ordinary_property_identity(property).property_owner();
        let identity = self
            .setter_identity_records
            .entry(setter)
            .or_insert_with(|| {
                hir::HirPropertyAccessorIdentity::setter(property, owner)
                    .expect("a checked property identifies its setter")
            })
            .id();
        let value = &self.property_setters[setter];
        hir::SemanticAccessorDescription {
            identity,
            callable: hir::SemanticCallable::CurrentSetter(setter),
            access: hir::SemanticMemberAccess {
                visibility: value.access.declared.into(),
                domain: value.access.lookup.0.clone(),
                accessible: self.property_accessor_is_accessible(
                    property,
                    &value.access,
                    Some(receiver),
                ),
            },
            implementation: hir::PropertyAccessorImplementationV1::from_source(
                value.implementation,
            ),
            annotations: hir::SemanticAnnotationTarget::Current(
                hir::SourceAnnotationTarget::Callable(hir::SourceAnnotationCallable::Setter(
                    setter,
                )),
            ),
            origin: hir::SemanticSourceLocation {
                source: self.source_identity(file).clone(),
                span: value.span,
            },
        }
    }
}
