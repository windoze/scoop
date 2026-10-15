use super::*;

impl Concretizer<'_> {
    pub(super) fn metadata_properties(
        &mut self,
        owner: concrete::TypeId,
        shape: export::StaticNominalShape<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let record = self.metadata_element(result);
        let constructor = shape.primary_constructor(self.dependencies);
        let properties = shape
            .properties(self.dependencies)
            .into_iter()
            .enumerate()
            .map(|(index, property)| {
                self.metadata_property(owner, property, index, constructor, record, origin)
            })
            .collect();
        self.metadata_array(result, properties, origin)
    }

    fn metadata_property(
        &mut self,
        owner: concrete::TypeId,
        property: export::StaticPropertyShape<'_>,
        index: usize,
        constructor: Option<export::StaticConstructorShape<'_>>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let id = self.metadata_identity(
            fields[0],
            format!("property:{}", property.identity()),
            origin,
        );
        let owner_value = self.type_reference_value(owner, fields[1], origin);
        let name = self.metadata_string(property.name().to_owned(), origin);
        let source = property.value_type_source();
        let ty = self.metadata_source_type(source, &self.type_arguments(owner));
        let ty = self.metadata_type_use(
            ty,
            property.type_annotations(self.dependencies),
            fields[3],
            origin,
        );
        let option = self.metadata_bool(self.metadata_is_declared_option(source), origin);
        let index = self.metadata_index(index, origin);
        let (getter, setter) = property.accessors(self.dependencies);
        let mutable = self.metadata_bool(setter.is_some(), origin);
        let visibility = self.metadata_visibility(fields[7], property.visibility(), origin);
        let annotations = self.metadata_annotations(
            property.annotations(self.dependencies).iter(),
            fields[8],
            origin,
        );
        let getter = self.metadata_accessor(getter, fields[9], origin);
        let setter = setter.map(|setter| {
            self.metadata_accessor(setter, self.metadata_optional_element(fields[10]), origin)
        });
        let setter = self.metadata_option(fields[10], setter, origin);
        let storage = match property.field_storage() {
            Some((id, storage)) => {
                let case = if matches!(storage, export::NominalFieldStorage::PropertyDelegate(_)) {
                    "Delegated"
                } else {
                    "Stored"
                };
                let id = self.metadata_identity(
                    self.metadata_variant(fields[11], case).1[0],
                    format!("field:{id}"),
                    origin,
                );
                self.metadata_case(fields[11], case, vec![id], origin)
            }
            None => self.metadata_case(fields[11], "Computed", Vec::new(), origin),
        };
        let reference = self.metadata_element(fields[12]);
        let overrides = property
            .overrides()
            .into_iter()
            .map(|id| self.metadata_identity(reference, format!("property:{id}"), origin))
            .collect();
        let overrides = self.metadata_array(fields[12], overrides, origin);
        let parameter = self.metadata_constructor_parameter(
            constructor,
            export::StaticParameterTarget::Property(property.identity()),
            fields[13],
            origin,
        );
        self.metadata_record(
            result,
            vec![
                id,
                owner_value,
                name,
                ty,
                option,
                index,
                mutable,
                visibility,
                annotations,
                getter,
                setter,
                storage,
                overrides,
                parameter,
            ],
            origin,
        )
    }

    fn metadata_accessor(
        &mut self,
        accessor: export::StaticAccessorShape<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let id =
            self.metadata_identity(fields[0], format!("accessor:{}", accessor.identity), origin);
        let visibility = self.metadata_visibility(fields[1], accessor.visibility, origin);
        let annotations = self.metadata_annotations(
            accessor.annotations(self.dependencies).iter(),
            fields[2],
            origin,
        );
        let kind = match accessor.implementation {
            export::PropertyAccessorImplementationV1::Storage => "Storage",
            export::PropertyAccessorImplementationV1::Constant => "Constant",
            export::PropertyAccessorImplementationV1::Body => "Body",
            export::PropertyAccessorImplementationV1::AbstractSlot => "Abstract",
            export::PropertyAccessorImplementationV1::StorageBody => "StorageBody",
        };
        let kind = self.metadata_case(fields[3], kind, Vec::new(), origin);
        self.metadata_record(result, vec![id, visibility, annotations, kind], origin)
    }
}
