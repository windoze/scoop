use super::*;

impl Fixture {
    pub fn builtin(&mut self, builtin: CoreBuiltinNominal) -> PersistentExactTypeId {
        let declaration = builtin.identity_record();
        let key = ExactTypeKey::Nominal(declaration.id());
        let exact = PersistentExactTypeId::from_key(&key).unwrap();
        self.source.graph.exacts.insert(exact, key);
        self.shapes.insert(
            exact,
            match builtin {
                CoreBuiltinNominal::Unit => ExactTypeFactShapeV1::Unit,
                CoreBuiltinNominal::Any => ExactTypeFactShapeV1::Reference,
            },
        );
        self.facts =
            CanonicalPersistentIdsV1::try_new(self.shapes.keys().copied().collect()).unwrap();
        exact
    }
}
