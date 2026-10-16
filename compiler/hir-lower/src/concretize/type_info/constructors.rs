use super::*;
use scoop_identity::CallableTemplateOrigin;

pub(super) fn constructor_identity(constructor: CallableTemplateOrigin) -> String {
    match constructor {
        CallableTemplateOrigin::Constructor(id) => format!("constructor:{id}"),
        CallableTemplateOrigin::VariantConstructor(id) => format!("variant-constructor:{id}"),
        _ => unreachable!("constructor views have constructor identities"),
    }
}

pub(super) fn parameter_identity(constructor: CallableTemplateOrigin, index: usize) -> String {
    format!("parameter:{}:{index}", constructor_identity(constructor))
}

pub(super) fn field_identity(field: export::StaticFieldIdentity) -> String {
    match field {
        export::StaticFieldIdentity::Field(id) => format!("field:{id}"),
        export::StaticFieldIdentity::VariantField(id) => format!("variant-field:{id}"),
    }
}

impl Concretizer<'_> {
    pub(super) fn metadata_constructors(
        &mut self,
        owner: concrete::TypeId,
        shape: export::StaticNominalShape<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let record = self.metadata_element(result);
        let constructors = shape
            .constructors(self.dependencies)
            .into_iter()
            .map(|constructor| self.metadata_constructor(owner, constructor, record, origin))
            .collect();
        self.metadata_array(result, constructors, origin)
    }

    pub(super) fn metadata_constructor(
        &mut self,
        owner: concrete::TypeId,
        constructor: export::StaticConstructorShape<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let id = self.metadata_identity(
            fields[0],
            constructor_identity(constructor.identity()),
            origin,
        );
        let owner_value = self.type_reference_value(owner, fields[1], origin);
        let kind = match constructor.identity() {
            CallableTemplateOrigin::VariantConstructor(id) => {
                let ty = self.metadata_variant(fields[2], "Variant").1[0];
                let id = self.metadata_identity(ty, format!("variant:{id}"), origin);
                self.metadata_case(fields[2], "Variant", vec![id], origin)
            }
            _ => self.metadata_case(
                fields[2],
                if constructor.is_primary() {
                    "Primary"
                } else {
                    "Secondary"
                },
                Vec::new(),
                origin,
            ),
        };
        let visibility = self.metadata_visibility(fields[3], constructor.visibility(), origin);
        let annotations = self.metadata_annotations(
            constructor.annotations(self.dependencies).iter(),
            fields[4],
            origin,
        );
        let parameter = self.metadata_element(fields[5]);
        let arguments = self.type_arguments(owner);
        let parameters = constructor
            .parameters()
            .map(|value| {
                self.metadata_parameter(
                    value,
                    constructor.identity(),
                    &arguments,
                    parameter,
                    origin,
                )
            })
            .collect();
        let parameters = self.metadata_array(fields[5], parameters, origin);
        self.metadata_record(
            result,
            vec![id, owner_value, kind, visibility, annotations, parameters],
            origin,
        )
    }

    fn metadata_parameter(
        &mut self,
        parameter: export::StaticConstructorParameter<'_>,
        constructor: CallableTemplateOrigin,
        arguments: &[concrete::TypeId],
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let id = self.metadata_identity(
            fields[0],
            parameter_identity(constructor, parameter.index()),
            origin,
        );
        let name = self.metadata_string(parameter.name().to_owned(), origin);
        let ty = self.metadata_source_type(parameter.value_type_source(), arguments);
        let ty = self.metadata_type_use(
            ty,
            parameter.type_annotations(self.dependencies),
            fields[2],
            origin,
        );
        let index = self.metadata_index(parameter.index(), origin);
        let annotations = self.metadata_annotations(
            parameter.annotations(self.dependencies).iter(),
            fields[4],
            origin,
        );
        let has_default = self.metadata_bool(parameter.default().is_some(), origin);
        let vararg = self.metadata_bool(parameter.is_vararg(), origin);
        let (kind, member) = match parameter.target() {
            export::StaticParameterTarget::Plain => ("None", Vec::new()),
            export::StaticParameterTarget::Field(id) => {
                let member = self.metadata_variant(fields[7], "Field").1[0];
                (
                    "Field",
                    vec![self.metadata_identity(member, field_identity(id), origin)],
                )
            }
            export::StaticParameterTarget::Property(id) => {
                let member = self.metadata_variant(fields[7], "Property").1[0];
                (
                    "Property",
                    vec![self.metadata_identity(member, format!("property:{id}"), origin)],
                )
            }
        };
        let member = self.metadata_case(fields[7], kind, member, origin);
        self.metadata_record(
            result,
            vec![
                id,
                name,
                ty,
                index,
                annotations,
                has_default,
                vararg,
                member,
            ],
            origin,
        )
    }

    pub(super) fn metadata_constructor_parameter(
        &mut self,
        constructor: Option<export::StaticConstructorShape<'_>>,
        target: export::StaticParameterTarget,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let parameter = constructor.and_then(|constructor| {
            constructor
                .parameters()
                .find(|parameter| parameter.target() == target)
                .map(|parameter| (constructor.identity(), parameter.index()))
        });
        let parameter = parameter.map(|(constructor, index)| {
            self.metadata_identity(
                self.metadata_optional_element(result),
                parameter_identity(constructor, index),
                origin,
            )
        });
        self.metadata_option(result, parameter, origin)
    }
}
