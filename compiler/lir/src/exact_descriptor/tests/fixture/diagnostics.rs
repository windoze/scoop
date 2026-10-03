use super::*;

pub(super) struct Graph {
    exact: PersistentExactTypeId,
    exact_key: ExactTypeKey,
    nominal: PersistentTypeId,
    source: SourceDeclarationKey,
    coordinate: ConeCoordinate,
}

impl Graph {
    pub(super) fn new(source: SourceDeclarationKey, exact_key: ExactTypeKey) -> Self {
        let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
        let exact = PersistentExactTypeId::from_key(&exact_key).unwrap();
        Self {
            exact,
            exact_key,
            nominal,
            source,
            coordinate: ConeCoordinate::reserved_single_file(),
        }
    }
}

impl ExactTypeDiagnosticGraph for Graph {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
        (id == self.exact).then_some(&self.exact_key)
    }

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
        (id == self.nominal).then_some(&self.source)
    }

    fn generated_nominal_key(&self, _id: PersistentTypeId) -> Option<&GeneratedNominalKey> {
        None
    }

    fn source_generic_type_declaration(
        &self,
        _id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        None
    }

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate> {
        (id == ConeIdentity::SINGLE_FILE).then_some(&self.coordinate)
    }
}
