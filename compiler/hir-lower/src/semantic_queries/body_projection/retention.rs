use super::*;

impl BodyQueries<'_> {
    pub(in crate::semantic_queries) fn retain_identities(self, lowerer: &mut Lowerer) {
        let Some(Ok(data)) = self.data.into_inner() else {
            return;
        };
        for (function, identity) in data.functions.iter() {
            if let Some(source) = identity.source_identity() {
                lowerer
                    .query_source_function_identities
                    .insert(function, source.clone());
            }
        }
        for (property, _) in self.lowerer.properties.iter() {
            lowerer
                .property_identity_records
                .insert(property, data.properties[property].clone());
        }
        for (getter, _) in self.lowerer.property_getters.iter() {
            lowerer.getter_identity_records.insert(
                getter,
                data.accessors
                    .get_getter(getter)
                    .expect("complete getter identity")
                    .clone(),
            );
        }
        for (setter, _) in self.lowerer.property_setters.iter() {
            lowerer.setter_identity_records.insert(
                setter,
                data.accessors
                    .get_setter(setter)
                    .expect("complete setter identity")
                    .clone(),
            );
        }
        for (constructor, _) in self.lowerer.struct_constructors.iter() {
            lowerer
                .struct_constructor_identity_records
                .insert(constructor, data.constructors[constructor].clone());
        }
        for (constructor, _) in self.lowerer.class_constructors.iter() {
            if let Some(source) = data.constructors[constructor].source_record() {
                lowerer
                    .class_constructor_identity_records
                    .insert(constructor, source.clone());
            }
        }
    }
}
