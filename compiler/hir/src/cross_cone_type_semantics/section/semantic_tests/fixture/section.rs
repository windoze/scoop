use super::*;
impl Fixture {
    pub fn section(
        &self,
        selected: Vec<SelectedExternalTypeUseV1>,
    ) -> CrossConeTypeSemanticsSectionV1 {
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            self.source.graph.records.values(),
            self.roots.iter().copied(),
            self,
        )
        .unwrap();
        let inheritance = self
            .edges
            .iter()
            .map(|edge| {
                NominalInheritanceInterfaceV1::try_new(
                    edge.clone(),
                    graph
                        .replay_nominal_domains(edge.owner())
                        .unwrap()
                        .to_record(),
                    CanonicalInheritanceConstructorsV1::try_new(
                        self.source.inheritance_interfaces.constructors[&edge.owner()]
                            .values()
                            .iter()
                            .map(|id| {
                                InheritanceConstructorInterfaceV1::try_new(
                                    self.source.inheritance_interfaces.constructor_sources[id]
                                        .clone(),
                                )
                                .unwrap()
                            })
                            .collect(),
                    )
                    .unwrap(),
                    CanonicalInheritanceSlotContractsV1::try_new(
                        self.slots.get(&edge.owner()).cloned().unwrap_or_default(),
                    )
                    .unwrap(),
                    self.source.inheritance_interfaces.members[&edge.owner()].clone(),
                    self.source.inheritance_interfaces.schemas[&edge.owner()].clone(),
                )
                .unwrap()
            })
            .collect();
        CrossConeTypeSemanticsSectionV1::new(
            CanonicalExactTypeFactsV1::try_new(
                self.facts
                    .values()
                    .iter()
                    .map(|exact| {
                        let (kind, gc) = match self.shapes[exact] {
                            ExactTypeFactShapeV1::Unit => (
                                ExactTypeKindV1::Value {
                                    zst: ZstStatus::ZeroSized,
                                },
                                ExactTypeGcV1::GcFree,
                            ),
                            _ => (
                                ExactTypeKindV1::Reference,
                                ExactTypeGcV1::ContainsManagedReferences,
                            ),
                        };
                        ExactTypeFactsV1::try_new(*exact, kind, gc).unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
            CanonicalNominalRepresentationSupportV1::try_new(
                self.representations
                    .values()
                    .iter()
                    .map(|id| self.source.graph.representations[id].clone())
                    .collect(),
            )
            .unwrap(),
            CanonicalNominalInheritanceInterfacesV1::try_new(inheritance).unwrap(),
            CanonicalProtectedDeclarationInterfacesV1::try_new(self.declarations.clone()).unwrap(),
            CanonicalProtectedCallableSourceInterfacesV1::try_new(self.protocols.clone()).unwrap(),
            CanonicalProtectedDefaultTemplatesV1::try_new(vec![]).unwrap(),
            self.origins(),
            CanonicalSelectedExternalTypeUsesV1::try_new(selected).unwrap(),
        )
    }
}
