use super::*;

impl<'a> ShapeDump<'a, '_> {
    pub(super) fn field(
        &mut self,
        field: StaticFieldShape<'a>,
        binders: &[HirSignatureBinder],
        parameters: &[TypeParamDecl],
        indent: usize,
    ) {
        let ty = self.type_name(
            &field.value_type().signature(self.module, binders).unwrap(),
            parameters,
        );
        writeln!(
            self.out,
            "{}field {}: {ty} {:?} {:?}",
            "  ".repeat(indent),
            field.name(),
            field.identity(),
            field.storage()
        )
        .unwrap();
        self.annotations(field.annotations(self.world), indent + 1);
    }

    pub(super) fn property(
        &mut self,
        property: StaticPropertyShape<'a>,
        binders: &[HirSignatureBinder],
        parameters: &[TypeParamDecl],
    ) {
        let ty = self.type_name(&property.value_type(binders).unwrap(), parameters);
        let visibility = match property.declaration() {
            StaticPropertyDeclaration::Current { declaration, .. } => {
                format!("{:?}", declaration.access.declared)
            }
            StaticPropertyDeclaration::Dependency(declaration) => {
                format!("{:?}", declaration.declared_visibility())
            }
        }
        .to_lowercase();
        let storage = match property.field_storage() {
            Some((field, NominalFieldStorage::PropertyBacking(_))) => format!("stored {field}"),
            Some((field, NominalFieldStorage::PropertyDelegate(_))) => format!("delegated {field}"),
            Some(_) => unreachable!("a logical property has only backing or delegate storage"),
            None => "no-field".to_owned(),
        };
        writeln!(
            self.out,
            "  {visibility} property {}: {ty} {} {storage}",
            property.name(),
            property.identity()
        )
        .unwrap();
        self.annotations(property.annotations(self.world), 2);
    }

    pub(super) fn constructor(
        &mut self,
        constructor: StaticConstructorShape<'a>,
        binders: &[HirSignatureBinder],
        parameters: &[TypeParamDecl],
        indent: usize,
    ) {
        writeln!(
            self.out,
            "{}constructor {:?}",
            "  ".repeat(indent),
            constructor.identity()
        )
        .unwrap();
        for parameter in constructor.parameters() {
            let ty = self.type_name(&parameter.value_type(binders).unwrap(), parameters);
            let default = match parameter.default() {
                None => String::new(),
                Some(StaticDefaultReference::Current { id, .. }) => {
                    format!(" default source{}", id.into_raw())
                }
                Some(StaticDefaultReference::Dependency { key, .. }) => {
                    format!(" default parameter{}", key.parameter_position())
                }
            };
            writeln!(
                self.out,
                "{}{}parameter {}: {ty} {:?}{default}",
                "  ".repeat(indent + 1),
                if parameter.is_vararg() { "vararg " } else { "" },
                parameter.name(),
                parameter.target()
            )
            .unwrap();
        }
    }
}
