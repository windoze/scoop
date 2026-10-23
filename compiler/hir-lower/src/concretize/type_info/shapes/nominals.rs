use super::*;

impl Concretizer<'_> {
    pub(super) fn metadata_nominal_shape(
        &mut self,
        target: concrete::TypeId,
        shape: export::StaticNominalShape<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let values = match shape.kind() {
            export::StaticNominalKind::Struct => {
                let storage = self.metadata_fields_array(target, shape, fields[0], origin);
                let constructors = self.metadata_constructors(target, shape, fields[1], origin);
                let interfaces = self.metadata_interfaces(target, shape, fields[2], origin);
                vec![storage, constructors, interfaces]
            }
            export::StaticNominalKind::Class | export::StaticNominalKind::Object => {
                let base = shape.base_class(self.dependencies).map(|base| {
                    let ty = self.metadata_source_type(base.source(), &self.type_arguments(target));
                    self.metadata_type_use(
                        ty,
                        shape.supertype_annotations(base, self.dependencies),
                        self.metadata_optional_element(fields[0]),
                        origin,
                    )
                });
                let base = self.metadata_option(fields[0], base, origin);
                let interfaces = self.metadata_interfaces(target, shape, fields[1], origin);
                let properties = self.metadata_properties(target, shape, fields[2], origin);
                let storage = self.metadata_fields_array(target, shape, fields[3], origin);
                let last = if shape.kind() == export::StaticNominalKind::Class {
                    self.metadata_constructors(target, shape, fields[4], origin)
                } else {
                    let host = shape.companion_host().map(|host| {
                        self.metadata_identity(
                            self.metadata_optional_element(fields[4]),
                            nominal_identity(host),
                            origin,
                        )
                    });
                    self.metadata_option(fields[4], host, origin)
                };
                let mut values = vec![base, interfaces, properties, storage, last];
                if shape.kind() == export::StaticNominalKind::Class {
                    values.push(self.metadata_bool(shape.is_final_class(), origin));
                }
                values
            }
            export::StaticNominalKind::Interface => {
                let parents = self.metadata_interfaces(target, shape, fields[0], origin);
                let properties = self.metadata_properties(target, shape, fields[1], origin);
                vec![parents, properties]
            }
            export::StaticNominalKind::Enum | export::StaticNominalKind::Intrinsic(_) => {
                unreachable!("enum and intrinsic types have dedicated descriptions")
            }
        };
        self.metadata_record(result, values, origin)
    }

    pub(super) fn metadata_interfaces(
        &mut self,
        target: concrete::TypeId,
        shape: export::StaticNominalShape<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let element = self.metadata_element(result);
        let arguments = self.type_arguments(target);
        let mut interfaces = shape
            .interfaces(self.dependencies)
            .map(|interface| {
                let ty = self.metadata_source_type(interface.source(), &arguments);
                self.metadata_type_use(
                    ty,
                    shape.supertype_annotations(interface, self.dependencies),
                    element,
                    origin,
                )
            })
            .collect::<Vec<_>>();
        for (index, declaration) in self
            .source
            .conditional_interfaces(shape.declaration())
            .iter()
            .enumerate()
        {
            if self.concrete_interface_condition_holds(declaration, &arguments) {
                let ty = self.lower_type(declaration.interface, &arguments);
                interfaces.push(self.metadata_type_use(
                    ty,
                    shape.conditional_interface_annotations(index, self.dependencies),
                    element,
                    origin,
                ));
            }
        }
        self.metadata_array(result, interfaces, origin)
    }
}
