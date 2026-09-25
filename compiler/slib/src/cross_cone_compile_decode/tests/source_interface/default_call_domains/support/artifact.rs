use super::*;

impl Builder {
    pub fn finish(self, templates: Vec<ExportDefaultTemplateV1>) -> Fixture {
        let mut foundation = base_hir_foundation();
        let mut types = vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
        ];
        let mut generics = vec![];
        let mut origins = vec![];
        let origin = DefinitionOrigin::new(
            self.source.clone(),
            SourceSpan::new(0, 12).unwrap(),
            &self.context,
        )
        .unwrap();
        for nominal in &self.nominals {
            let subject = match nominal.id {
                SourceNominalId::Concrete(id) => {
                    types.push(CborIdentityRecord::from_key(nominal.key.clone()).unwrap());
                    DefinitionOriginSubject::Type(id)
                }
                SourceNominalId::GenericTemplate(id) => {
                    generics.push(CborIdentityRecord::from_key(nominal.key.clone()).unwrap());
                    DefinitionOriginSubject::GenericType(id)
                }
            };
            origins.push(DefinitionOriginRecord::new(subject, origin.clone()));
        }
        let mut contexts = vec![
            CborIdentityRecord::<PersistentSourceContextId, _>::from_key(self.context).unwrap(),
        ];
        for function in &self.functions {
            origins.push(DefinitionOriginRecord::new(
                DefinitionOriginSubject::Function(function.id()),
                origin.clone(),
            ));
            contexts.push(
                CborIdentityRecord::from_key(SourceContextKey::Callable {
                    source: self.source.clone(),
                    owner: CallableOwner::Function(function.id()),
                })
                .unwrap(),
            );
        }
        foundation.set_types(types).unwrap();
        foundation.set_generic_types(generics).unwrap();
        foundation.set_functions(self.functions).unwrap();
        foundation
            .set_dispatch_slots(self.slots.into_values().collect())
            .unwrap();
        foundation
            .set_sources(vec![
                SourceRecord::from_utf8(self.source, "declarations", [0, 12]).unwrap(),
            ])
            .unwrap();
        foundation.set_source_contexts(contexts).unwrap();
        foundation.set_definition_origins(origins).unwrap();
        let mut public = vec![];
        let mut support = vec![];
        for record in self.callables {
            if let Some(access) = self.public.get(&record.declaration()) {
                public.push(CallableInterfaceRecordV1::from_declaration(record, *access).unwrap());
            } else {
                support.push(record);
            }
        }
        let (public_nominals, support_nominals): (Vec<_>, Vec<_>) = self
            .nominals
            .iter()
            .map(|nominal| nominal.record(&self.nominals))
            .partition(|record| {
                record.declaration_details().declared_visibility() == Visibility::Public
            });
        let interface = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(vec![]).unwrap(),
            CanonicalNominalInterfacesV1::with_support(public_nominals, support_nominals).unwrap(),
            CanonicalCallableInterfacesV1::with_support(public, support).unwrap(),
            CanonicalPropertyInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalTypeAliasInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(templates).unwrap(),
            CanonicalExportConstValuesV1::try_new(vec![]).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
            CanonicalExternalHirReferencesV1::try_new(vec![]).unwrap(),
        );
        Fixture {
            cone: self.cone,
            foundation,
            interface,
        }
    }
}

impl Fixture {
    pub fn validate(&self) -> Result<(), Error> {
        let mut interface = self.interface.clone();
        let bytes = cross_cone_artifact_for_with_hir_foundation(
            self.cone.clone(),
            vec![],
            &self.foundation,
            encode(&interface.index_for_wire().unwrap()).unwrap(),
        );
        let front = declaration_front(&bytes);
        let current = front.graph.identity();
        CanonicalCrossConeHirSurfaceAuthority::new(
            current,
            &front.identities,
            &front.foundations.hir,
            &front.hir_interface,
            vec![],
        )
        .validate_default_call_domains()
    }

    pub fn failure(&self) -> Error {
        let Err(mut error) = self.validate() else {
            panic!("forged source call domains must fail")
        };
        loop {
            match error {
                Error::Template { source, .. } | Error::Declaration { source, .. } => {
                    error = *source
                }
                leaf => return leaf,
            }
        }
    }

    pub fn change_callable(
        &mut self,
        declaration: CallableTemplateOrigin,
        update: impl FnOnce(&CallableDeclarationRecordV1) -> CallableDeclarationRecordV1,
    ) {
        let original = self.interface.callable_interfaces();
        let changed = update(original.declaration(declaration).unwrap());
        let public = original
            .records()
            .iter()
            .map(|record| {
                if record.declaration() == declaration {
                    CallableInterfaceRecordV1::from_declaration(changed.clone(), record.access())
                        .unwrap()
                } else {
                    record.clone()
                }
            })
            .collect();
        let support = original
            .support_records()
            .iter()
            .map(|record| {
                if record.declaration() == declaration {
                    changed.clone()
                } else {
                    record.clone()
                }
            })
            .collect();
        self.replace(
            CanonicalCallableInterfacesV1::with_support(public, support).unwrap(),
            self.interface.default_templates().clone(),
        );
    }

    pub(super) fn replace(
        &mut self,
        callables: CanonicalCallableInterfacesV1,
        templates: CanonicalExportDefaultTemplatesV1,
    ) {
        let i = &self.interface;
        self.interface = CrossConeHirInterfaceSectionV1::new(
            i.public_bindings().clone(),
            i.nominal_interfaces().clone(),
            callables,
            i.property_interfaces().clone(),
            i.type_aliases().clone(),
            i.source_interfaces().clone(),
            templates,
            i.constants().clone(),
            i.definition_sources().clone(),
            i.external_references().clone(),
        );
    }
}
