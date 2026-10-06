use super::*;

impl Lowerer {
    pub(super) fn coding_dependencies(
        &mut self,
        owner: Owner,
        interface: TypeId,
    ) -> Vec<(String, CodecDependency)> {
        match owner {
            Owner::Struct(_) => {
                let ty = self.owner_ty(owner);
                self.struct_fields(ty)
                    .expect("a struct owner has its declared fields")
                    .fields
                    .into_iter()
                    .filter_map(|field| {
                        self.is_subtype(field.ty, interface).then_some((
                            field.name,
                            CodecDependency::Field(field.reference, field.ty),
                        ))
                    })
                    .collect()
            }
            Owner::Class(class) => {
                let fields = self.classes[class].fields.clone();
                fields
                    .into_iter()
                    .filter_map(|field| {
                        let field = &self.class_fields[field];
                        if !matches!(field.source, hir::ClassFieldSource::PrimaryParameter(_)) {
                            return None;
                        }
                        let id = field.property;
                        let property = &self.properties[id];
                        let (name, ty, mutable) = (
                            property.name.clone(),
                            property.ty,
                            property.capability.setter().is_some(),
                        );
                        if mutable || !self.is_subtype(ty, interface) {
                            return None;
                        }
                        Some((name, CodecDependency::Property(id, ty)))
                    })
                    .collect()
            }
            Owner::Enum(_) | Owner::Object(_) => Vec::new(),
            Owner::Interface(_) => unreachable!("an interface has no synthesized method body"),
        }
    }

    pub(super) fn coding_dependency_value(
        &mut self,
        dependency: &CodecDependency,
        span: Span,
    ) -> Option<hir::Expr> {
        let receiver = self
            .lower_current_this(span)
            .expect("a codec receiver remains available to nested closures");
        match *dependency {
            CodecDependency::Field(field, ty) => Some(self.coding_expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(receiver),
                    field,
                },
                ty,
                span,
            )),
            CodecDependency::Property(property, ty) => {
                let Type::Class(application) = self.types[receiver.ty] else {
                    unreachable!("the dependency belongs to a class")
                };
                self.lower_property_read(
                    property,
                    Some(hir::MethodOwnerApplication::Class(application)),
                    Some(receiver),
                    ty,
                    span,
                )
            }
        }
    }
}
