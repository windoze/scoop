use super::*;

pub(super) struct EnumFixture {
    pub(super) owner: PersistentTypeId,
    pub(super) variants: Vec<PersistentEnumVariantId>,
}

pub(super) struct FixtureBuilder {
    origin: ConeIdentity,
    pub(super) existing: ExistingProtocolFixture,
    next_name: u32,
    types: Vec<TypeRecord>,
    generic_types: Vec<GenericTypeRecord>,
    functions: Vec<FunctionRecord>,
    generic_functions: Vec<GenericFunctionRecord>,
    constructors: Vec<ConstructorRecord>,
    variants: Vec<VariantRecord>,
    variant_fields: Vec<VariantFieldRecord>,
    exact_types: Vec<ExactTypeRecord>,
    dispatch_slots: Vec<DispatchRecord>,
    origins: Vec<DefinitionOriginRecord>,
}

impl FixtureBuilder {
    pub(super) fn new(existing: ExistingProtocolFixture, origin: ConeIdentity) -> Self {
        Self {
            origin,
            existing,
            next_name: 0,
            types: vec![
                CoreBuiltinNominal::Unit.identity_record(),
                CoreBuiltinNominal::Any.identity_record(),
            ],
            generic_types: Vec::new(),
            functions: Vec::new(),
            generic_functions: Vec::new(),
            constructors: Vec::new(),
            variants: Vec::new(),
            variant_fields: Vec::new(),
            exact_types: Vec::new(),
            dispatch_slots: Vec::new(),
            origins: Vec::new(),
        }
    }

    pub(super) fn concrete_type(&mut self) -> PersistentTypeId {
        let record: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(self.origin, DefinitionOwnerChain::top_level()),
            self.name(),
            SourceNominalKind::Class,
            0,
        ))
        .unwrap();
        let id = record.id();
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::Type(id),
        ));
        self.types.push(record);
        id
    }

    pub(super) fn concrete_nominal(&mut self, kind: SourceNominalKind) -> CoreProtocolEntryV1 {
        let record: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(self.origin, DefinitionOwnerChain::top_level()),
            self.name(),
            kind,
            0,
        ))
        .unwrap();
        let id = record.id();
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::Type(id),
        ));
        self.types.push(record);
        concrete_entry(id)
    }

    pub(super) fn generic_nominal(&mut self, kind: SourceNominalKind) -> CoreProtocolEntryV1 {
        let record: GenericTypeRecord =
            CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                site(self.origin, DefinitionOwnerChain::top_level()),
                self.name(),
                kind,
                1,
            ))
            .unwrap();
        let id = record.id();
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::GenericType(id),
        ));
        self.generic_types.push(record);
        generic_entry(id)
    }

    pub(super) fn function(
        &mut self,
        effect: scoop_identity::Effect,
    ) -> (CoreProtocolEntryV1, PersistentFunctionId) {
        let unit = CoreBuiltinNominal::Unit.identity_record().id();
        self.function_with_owner_signature(
            None,
            SignatureCallableShape::new(effect, None, Vec::new(), SignatureTypeKey::Nominal(unit)),
        )
    }

    pub(super) fn function_with_owner_signature(
        &mut self,
        owner: Option<DefinitionOwnerAtom>,
        signature: SignatureCallableShape,
    ) -> (CoreProtocolEntryV1, PersistentFunctionId) {
        let record: FunctionRecord = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site(
                self.origin,
                owner.map_or_else(DefinitionOwnerChain::top_level, |owner| {
                    DefinitionOwnerChain::from_outer_to_inner(vec![owner])
                }),
            ),
            self.name(),
            0,
            None,
            signature.parameters().to_vec(),
        ))
        .unwrap();
        let id = record.id();
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::Function(id),
        ));
        self.functions.push(record);
        (
            CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
                CoreProtocolCallableDefinitionV1::Function(id),
                signature,
            )),
            id,
        )
    }

    pub(super) fn operation(
        &mut self,
        owner: Option<DefinitionOwnerAtom>,
        own_type_parameter_count: u32,
        signature: SignatureCallableShape,
    ) -> CoreProtocolEntryV1 {
        let source = SourceDeclarationKey::function(
            site(
                self.origin,
                owner.map_or_else(DefinitionOwnerChain::top_level, |owner| {
                    DefinitionOwnerChain::from_outer_to_inner(vec![owner])
                }),
            ),
            self.name(),
            own_type_parameter_count,
            None,
            signature.parameters().to_vec(),
        );
        if own_type_parameter_count == 0 {
            let record: FunctionRecord = CborIdentityRecord::from_key(source).unwrap();
            let id = record.id();
            self.origins.push(origin_record(
                self.origin,
                DefinitionOriginSubject::Function(id),
            ));
            self.functions.push(record);
            CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
                CoreProtocolCallableDefinitionV1::Function(id),
                signature,
            ))
        } else {
            let record: GenericFunctionRecord = CborIdentityRecord::from_key(source).unwrap();
            let id = record.id();
            self.origins.push(origin_record(
                self.origin,
                DefinitionOriginSubject::GenericFunction(id),
            ));
            self.generic_functions.push(record);
            CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
                CoreProtocolCallableDefinitionV1::GenericFunction(id),
                signature,
            ))
        }
    }

    pub(super) fn constructor(
        &mut self,
        owner: PersistentTypeId,
        parameters: Vec<SignatureTypeKey>,
    ) -> CoreProtocolEntryV1 {
        let record: ConstructorRecord =
            CborIdentityRecord::from_key(SourceDeclarationKey::constructor(
                site(
                    self.origin,
                    DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                        owner,
                    )]),
                ),
                parameters.clone(),
            ))
            .unwrap();
        let id = record.id();
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::Constructor(id),
        ));
        self.constructors.push(record);
        CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
            CoreProtocolCallableDefinitionV1::Constructor(id),
            SignatureCallableShape::new(
                scoop_identity::Effect::Ordinary,
                None,
                parameters,
                SignatureTypeKey::Nominal(owner),
            ),
        ))
    }

    pub(super) fn concrete_enum_with_variants(&mut self, count: usize) -> EnumFixture {
        let record: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(self.origin, DefinitionOwnerChain::top_level()),
            self.name(),
            SourceNominalKind::Enum,
            0,
        ))
        .unwrap();
        let owner = record.id();
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::Type(owner),
        ));
        let owner_key = record.key().clone();
        self.types.push(record);
        let variants = (0..count)
            .map(|_| {
                let record: VariantRecord = CborIdentityRecord::from_key(
                    EnumVariantIdentityKey::source(&owner_key, self.name()).unwrap(),
                )
                .unwrap();
                let id = record.id();
                self.origins.push(origin_record(
                    self.origin,
                    DefinitionOriginSubject::EnumVariant(id),
                ));
                self.variants.push(record);
                id
            })
            .collect();
        EnumFixture { owner, variants }
    }

    pub(super) fn exact_nominal(&mut self, owner: PersistentTypeId) -> PersistentExactTypeId {
        let record: ExactTypeRecord =
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap();
        let id = record.id();
        self.exact_types.push(record);
        id
    }

    pub(super) fn exact_application(
        &mut self,
        origin: PersistentGenericTypeId,
        argument_owner: PersistentTypeId,
    ) -> PersistentExactTypeId {
        let argument = self.exact_nominal(argument_owner);
        let record: ExactTypeRecord =
            CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
                origin,
                arguments: NonEmptyVec::new(vec![argument]).unwrap(),
            })
            .unwrap();
        let id = record.id();
        self.exact_types.push(record);
        id
    }

    pub(super) fn dispatch(&mut self, function: PersistentFunctionId) -> CoreProtocolEntryV1 {
        let record: DispatchRecord =
            CborIdentityRecord::from_key(DispatchSlotKey::interface_method(function)).unwrap();
        let id = record.id();
        self.dispatch_slots.push(record);
        CoreProtocolEntryV1::DispatchSlot(id)
    }

    pub(super) fn name(&mut self) -> CanonicalIdentifier {
        let text = format!("protocol{}", self.next_name);
        let name = CanonicalIdentifier::new(&text).unwrap();
        self.next_name += 1;
        name
    }

    pub(super) fn install(mut self, foundation: &mut CanonicalHirFoundation) {
        self.types.push(self.existing.string.clone());
        self.generic_types.push(self.existing.option.clone());
        self.variants.push(self.existing.option_some.clone());
        self.variants.push(self.existing.option_none.clone());
        self.variant_fields
            .push(self.existing.option_some_payload.clone());
        self.exact_types.append(&mut self.existing.exact_types);
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::Type(self.existing.string.id()),
        ));
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::GenericType(self.existing.option.id()),
        ));
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::EnumVariant(self.existing.option_some.id()),
        ));
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::EnumVariantField(self.existing.option_some_payload.id()),
        ));
        self.origins.push(origin_record(
            self.origin,
            DefinitionOriginSubject::EnumVariant(self.existing.option_none.id()),
        ));

        foundation.set_types(self.types).unwrap();
        foundation.set_generic_types(self.generic_types).unwrap();
        foundation.set_functions(self.functions).unwrap();
        foundation
            .set_generic_functions(self.generic_functions)
            .unwrap();
        foundation.set_constructors(self.constructors).unwrap();
        foundation.set_enum_variants(self.variants).unwrap();
        foundation
            .set_enum_variant_fields(self.variant_fields)
            .unwrap();
        foundation.set_exact_types(self.exact_types).unwrap();
        foundation.set_dispatch_slots(self.dispatch_slots).unwrap();
        foundation.set_definition_origins(self.origins).unwrap();
    }
}
