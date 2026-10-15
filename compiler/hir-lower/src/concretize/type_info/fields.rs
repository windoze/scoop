use super::*;

impl Concretizer<'_> {
    pub(super) fn metadata_is_declared_option(
        &self,
        ty: export::StaticShapeTypeSource<'_>,
    ) -> bool {
        let nominal = match ty {
            export::StaticShapeTypeSource::Hir(ty) => match self.source.types[ty] {
                export::Type::Enum(application) => {
                    Some(self.source.enum_applications[application].template)
                }
                _ => None,
            },
            export::StaticShapeTypeSource::Published(
                scoop_identity::SignatureTypeKey::NominalApplication { origin, .. },
            ) => Some(export::SourceNominalId::GenericTemplate(*origin)),
            export::StaticShapeTypeSource::Published(_) => None,
        };
        let option = match self.core {
            export::CoreProtocols::Defined(core) => {
                self.source.nominal_identities[core.option.enumeration()].declaration_id()
            }
            export::CoreProtocols::Imported(core) => {
                export::SourceNominalId::GenericTemplate(core.option().option().persistent())
            }
        };
        nominal == Some(option)
    }

    pub(super) fn metadata_fields_array(
        &mut self,
        owner: concrete::TypeId,
        shape: export::StaticNominalShape<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let record = self.metadata_element(result);
        let constructor = shape.primary_constructor(self.dependencies);
        let fields = shape
            .fields()
            .enumerate()
            .map(|(index, field)| {
                self.metadata_field(owner, field, (index, false), constructor, record, origin)
            })
            .collect();
        self.metadata_array(result, fields, origin)
    }

    pub(super) fn metadata_field(
        &mut self,
        owner: concrete::TypeId,
        field: export::StaticFieldShape<'_>,
        position: (usize, bool),
        constructor: Option<export::StaticConstructorShape<'_>>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let id = self.metadata_identity(
            fields[0],
            constructors::field_identity(field.identity()),
            origin,
        );
        let owner_value = self.type_reference_value(owner, fields[1], origin);
        let (index, positional) = position;
        let name = if positional {
            let index = self.metadata_index(index, origin);
            self.metadata_case(fields[2], "Positional", vec![index], origin)
        } else {
            let name = self.metadata_string(field.name().to_owned(), origin);
            self.metadata_case(fields[2], "Named", vec![name], origin)
        };
        let source = field.value_type().source();
        let ty = self.metadata_source_type(source, &self.type_arguments(owner));
        let ty = self.metadata_type_use(
            ty,
            field.type_annotations(self.dependencies),
            fields[3],
            origin,
        );
        let option = self.metadata_bool(self.metadata_is_declared_option(source), origin);
        let index = self.metadata_index(index, origin);
        let annotations = self.metadata_annotations(
            field.annotations(self.dependencies).iter(),
            fields[6],
            origin,
        );
        let visibility = if field.storage() == export::NominalFieldStorage::Declared {
            export::DeclaredVisibilityV1::Public
        } else {
            export::DeclaredVisibilityV1::Private
        };
        let visibility = self.metadata_visibility(fields[7], visibility, origin);
        let storage = self.metadata_storage(field.storage(), fields[8], origin);
        let parameter = self.metadata_constructor_parameter(
            constructor,
            match field.storage() {
                export::NominalFieldStorage::PropertyBacking(id) => {
                    export::StaticParameterTarget::Property(id)
                }
                _ => export::StaticParameterTarget::Field(field.identity()),
            },
            fields[9],
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
                annotations,
                visibility,
                storage,
                parameter,
            ],
            origin,
        )
    }

    fn metadata_storage(
        &mut self,
        storage: export::NominalFieldStorage,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        use export::NominalFieldStorage as S;
        let (name, property) = match storage {
            S::Declared => ("Declared", None),
            S::EnclosingInstance => ("Outer", None),
            S::CapturedEnvironment => ("Capture", None),
            S::Generated => ("Generated", None),
            S::PropertyBacking(property) => ("Backing", Some(property)),
            S::PropertyDelegate(property) => ("Delegate", Some(property)),
        };
        let values = property
            .map(|id| {
                self.metadata_identity(
                    self.metadata_variant(result, name).1[0],
                    format!("property:{id}"),
                    origin,
                )
            })
            .into_iter()
            .collect();
        self.metadata_case(result, name, values, origin)
    }
}
