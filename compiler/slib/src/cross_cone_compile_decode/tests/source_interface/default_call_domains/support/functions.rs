use super::*;

impl Builder {
    pub fn function(
        &mut self,
        owner: SourceNominalId,
        name: &str,
        visibility: Visibility,
        modality: Modality,
        dispatch: Dispatch,
        parameters: &[SignatureTypeKey],
    ) -> CallableTemplateOrigin {
        let nominal = self
            .nominals
            .iter_mut()
            .find(|record| record.id == owner)
            .unwrap();
        let mut owners = nominal.key.owners().owners().to_vec();
        owners.push(owner_atom(owner));
        let function = CborIdentityRecord::<PersistentFunctionId, _>::from_key(
            SourceDeclarationKey::function(
                site(self.cone.identity(), owners),
                CanonicalIdentifier::new(name).unwrap(),
                0,
                None,
                parameters.to_vec(),
            ),
        )
        .unwrap();
        let id = CallableTemplateOrigin::Function(function.id());
        let slot = match dispatch {
            Dispatch::Direct => vec![],
            Dispatch::Inherited(base) => self
                .callables
                .iter()
                .find(|record| record.declaration() == base)
                .unwrap()
                .slot_relations()
                .values()
                .to_vec(),
            Dispatch::Virtual | Dispatch::Interface => {
                let key = match dispatch {
                    Dispatch::Interface => DispatchSlotKey::interface_method(function.id()),
                    _ => DispatchSlotKey::virtual_method(function.id()),
                };
                let record =
                    CborIdentityRecord::<PersistentDispatchSlotId, _>::from_key(key).unwrap();
                let slot = record.id();
                self.slots.insert(slot, record);
                vec![slot]
            }
        };
        nominal
            .members
            .push(NestedSourceMemberRefV1::Function(function.id()));
        self.callables.push(
            CallableDeclarationRecordV1::try_new(
                id,
                PublicDeclarationOwnerV1::Nominal(owner),
                CanonicalBinderListV1::try_new(vec![]).unwrap(),
                None,
                CanonicalSourceParameterShapesV1::try_new(
                    parameters
                        .iter()
                        .enumerate()
                        .map(|(index, ty)| {
                            SourceParameterShapeV1::new(
                                CanonicalIdentifier::new(&format!("value{index}")).unwrap(),
                                ty.clone(),
                            )
                        })
                        .collect(),
                )
                .unwrap(),
                self.token.clone(),
                scoop_effects(),
                modality,
                visibility,
                CanonicalPersistentIdsV1::try_new(slot).unwrap(),
            )
            .unwrap(),
        );
        self.functions.push(function);
        id
    }

    pub fn public(&mut self, function: CallableTemplateOrigin, access: PublicLookupAccessV1) {
        self.public.insert(function, access);
    }
    pub(super) fn body_origin(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> ExportDefinitionSourceV1 {
        let CallableTemplateOrigin::Function(id) = declaration else {
            panic!("source method")
        };
        ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(
                self.source.clone(),
                SourceSpan::new(0, 12).unwrap(),
                &SourceContextKey::Callable {
                    source: self.source.clone(),
                    owner: CallableOwner::Function(id),
                },
            )
            .unwrap(),
        )
    }
}
