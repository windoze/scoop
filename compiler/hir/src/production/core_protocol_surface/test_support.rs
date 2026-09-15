use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerAtom,
    DefinitionOwnerChain, DispatchSlotKey, EnumVariantFieldKey, EnumVariantFieldSelector,
    EnumVariantIdentityKey, ExactTypeKey, NonEmptyVec, NormalizedSourcePath, PackagePath,
    PersistentConstructorId, PersistentDispatchSlotId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentTypeId, SignatureCallableShape,
    SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};

use super::*;
use crate::{CanonicalHirFoundation, CoreProtocolCallableDefinitionV1};

type TypeRecord = CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>;
type GenericTypeRecord = CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>;
type FunctionRecord = CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>;
type GenericFunctionRecord = CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey>;
type ConstructorRecord = CborIdentityRecord<PersistentConstructorId, SourceDeclarationKey>;
type VariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type VariantFieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;
type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type DispatchRecord = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;

pub(crate) struct ExistingProtocolFixture {
    pub string: TypeRecord,
    pub option: GenericTypeRecord,
    pub option_some: VariantRecord,
    pub option_some_payload: VariantFieldRecord,
    pub option_none: VariantRecord,
    pub exact_types: Vec<ExactTypeRecord>,
}

pub(crate) fn standalone() -> (CoreCompilerProtocolSurfaceV1, CanonicalHirFoundation) {
    let string: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(DefinitionOwnerChain::top_level()),
        CanonicalIdentifier::new("String").unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap();
    let option: GenericTypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(DefinitionOwnerChain::top_level()),
        CanonicalIdentifier::new("Option").unwrap(),
        SourceNominalKind::Enum,
        1,
    ))
    .unwrap();
    let option_some: VariantRecord = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(option.key(), CanonicalIdentifier::new("Some").unwrap())
            .unwrap(),
    )
    .unwrap();
    let option_some_payload: VariantFieldRecord =
        CborIdentityRecord::from_key(EnumVariantFieldKey::new(
            option_some.id(),
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ))
        .unwrap();
    let option_none: VariantRecord = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(option.key(), CanonicalIdentifier::new("None").unwrap())
            .unwrap(),
    )
    .unwrap();
    let mut foundation = CanonicalHirFoundation::empty();
    let surface = install(
        &mut foundation,
        ExistingProtocolFixture {
            string,
            option,
            option_some,
            option_some_payload,
            option_none,
            exact_types: Vec::new(),
        },
    );
    (surface, foundation)
}

pub(crate) fn install(
    foundation: &mut CanonicalHirFoundation,
    existing: ExistingProtocolFixture,
) -> CoreCompilerProtocolSurfaceV1 {
    let mut builder = FixtureBuilder::new(existing);

    let integers: [CoreProtocolEntryV1; 8] =
        std::array::from_fn(|_| builder.concrete_nominal(SourceNominalKind::Struct));
    let boolean = builder.concrete_nominal(SourceNominalKind::Struct);
    let array = builder.generic_nominal(SourceNominalKind::Class);
    let mutable_array = builder.generic_nominal(SourceNominalKind::Class);
    let ptr = builder.generic_nominal(SourceNominalKind::Struct);
    let fun_ptr = builder.generic_nominal(SourceNominalKind::Struct);
    let fundamental_types = CoreFundamentalTypeProtocolV1(product([
        concrete_entry(CoreBuiltinNominal::Unit.identity_record().id()),
        integers[0].clone(),
        integers[1].clone(),
        integers[2].clone(),
        integers[3].clone(),
        integers[4].clone(),
        integers[5].clone(),
        integers[6].clone(),
        integers[7].clone(),
        boolean,
        concrete_entry(builder.existing.string.id()),
        array,
        mutable_array,
        ptr.clone(),
        fun_ptr.clone(),
    ]));

    let option_protocol = CoreOptionProtocolV1(product([
        generic_entry(builder.existing.option.id()),
        CoreProtocolEntryV1::EnumVariant(builder.existing.option_some.id()),
        CoreProtocolEntryV1::EnumVariantField(builder.existing.option_some_payload.id()),
        CoreProtocolEntryV1::EnumVariant(builder.existing.option_none.id()),
    ]));
    let option_type = builder.existing.option.id();
    let string_type = builder.existing.string.id();

    let iterator = builder.generic_nominal(SourceNominalKind::Interface);
    let iterator_id = match iterator {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("the fixture iterator is generic"),
    };
    let (next, next_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(iterator_id)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            Vec::new(),
            signature_application(option_type, SignatureTypeKey::Binder { depth: 0, index: 0 }),
        ),
    );
    let iteration_protocol =
        CoreIterationProtocolV1(product([iterator, next, builder.dispatch(next_id)]));

    let exception_classes: [PersistentTypeId; 6] = std::array::from_fn(|_| builder.concrete_type());
    let exception_constructors =
        exception_classes.map(|class| builder.constructor(class, Vec::new()));
    let initialization_cycle_thrower = builder
        .function_with_owner_signature(
            None,
            SignatureCallableShape::new(
                scoop_identity::Effect::Ordinary,
                None,
                vec![SignatureTypeKey::Nominal(string_type)],
                SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
            ),
        )
        .0;
    let exception_protocol = CoreExceptionProtocolV1(product([
        concrete_entry(exception_classes[0]),
        exception_constructors[0].clone(),
        concrete_entry(exception_classes[1]),
        exception_constructors[1].clone(),
        concrete_entry(exception_classes[2]),
        exception_constructors[2].clone(),
        concrete_entry(exception_classes[3]),
        exception_constructors[3].clone(),
        concrete_entry(exception_classes[4]),
        exception_constructors[4].clone(),
        concrete_entry(exception_classes[5]),
        exception_constructors[5].clone(),
        initialization_cycle_thrower,
    ]));

    let continuation = builder.generic_nominal(SourceNominalKind::Interface);
    let continuation_id = generic_protocol_id(&continuation);
    let (resume, resume_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(continuation_id)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        ),
    );
    let (resume_exception, resume_exception_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(continuation_id)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![SignatureTypeKey::Nominal(exception_classes[0])],
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        ),
    );
    let suspend_task = builder.generic_nominal(SourceNominalKind::Interface);
    let (run, run_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(generic_protocol_id(
            &suspend_task,
        ))),
        SignatureCallableShape::new(
            scoop_identity::Effect::Suspend,
            None,
            Vec::new(),
            SignatureTypeKey::Binder { depth: 0, index: 0 },
        ),
    );
    let suspend_registration = builder.generic_nominal(SourceNominalKind::Interface);
    let (register, register_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(generic_protocol_id(
            &suspend_registration,
        ))),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![signature_application(
                continuation_id,
                SignatureTypeKey::Binder { depth: 0, index: 0 },
            )],
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        ),
    );
    let (start, _) = builder.function(scoop_identity::Effect::Ordinary);
    let (suspend, _) = builder.function(scoop_identity::Effect::Suspend);
    let mut coroutine_protocol = CoreCoroutineProtocolV1(product([
        continuation,
        resume,
        builder.dispatch(resume_id),
        resume_exception,
        builder.dispatch(resume_exception_id),
        suspend_task,
        run,
        builder.dispatch(run_id),
        suspend_registration,
        register,
        builder.dispatch(register_id),
        start,
        suspend,
    ]));

    let pinned = builder.generic_nominal(SourceNominalKind::Struct);
    let handle = builder.generic_nominal(SourceNominalKind::Struct);
    let ffi_callables: [CoreProtocolEntryV1; 15] =
        std::array::from_fn(|_| builder.function(scoop_identity::Effect::Ordinary).0);
    let mut ffi_protocol = CoreFfiProtocolV1(product([
        ptr,
        fun_ptr,
        pinned,
        handle,
        ffi_callables[0].clone(),
        ffi_callables[1].clone(),
        ffi_callables[2].clone(),
        ffi_callables[3].clone(),
        ffi_callables[4].clone(),
        ffi_callables[5].clone(),
        ffi_callables[6].clone(),
        ffi_callables[7].clone(),
        ffi_callables[8].clone(),
        ffi_callables[9].clone(),
        ffi_callables[10].clone(),
        ffi_callables[11].clone(),
        ffi_callables[12].clone(),
        ffi_callables[13].clone(),
        ffi_callables[14].clone(),
    ]));

    let callback = builder.generic_nominal(SourceNominalKind::Struct);
    let mode = builder.concrete_enum_with_variants(2);
    let state = builder.concrete_enum_with_variants(4);
    let option_id = builder.existing.option.id();
    let failure_exact = builder.exact_application(option_id, exception_classes[0]);
    let callback_callables: [CoreProtocolEntryV1; 5] =
        std::array::from_fn(|_| builder.function(scoop_identity::Effect::Ordinary).0);
    let mut foreign_callback_protocol = CoreForeignCallbackProtocolV1(product([
        callback,
        concrete_entry(mode.owner),
        CoreProtocolEntryV1::EnumVariant(mode.variants[0]),
        CoreProtocolEntryV1::EnumVariant(mode.variants[1]),
        concrete_entry(state.owner),
        CoreProtocolEntryV1::EnumVariant(state.variants[0]),
        CoreProtocolEntryV1::EnumVariant(state.variants[1]),
        CoreProtocolEntryV1::EnumVariant(state.variants[2]),
        CoreProtocolEntryV1::EnumVariant(state.variants[3]),
        CoreProtocolEntryV1::ExactType(failure_exact),
        callback_callables[0].clone(),
        callback_callables[1].clone(),
        callback_callables[2].clone(),
        callback_callables[3].clone(),
        callback_callables[4].clone(),
    ]));

    let source_location = builder.concrete_nominal(SourceNominalKind::Struct);
    let current = builder.function(scoop_identity::Effect::Ordinary).0;
    let mut source_location_protocol =
        CoreSourceLocationProtocolV1(product([source_location, current]));

    let signature_surface = CoreCompilerProtocolSurfaceV1 {
        fundamental_types: fundamental_types.clone(),
        option_protocol: option_protocol.clone(),
        iteration_protocol: iteration_protocol.clone(),
        exception_protocol: exception_protocol.clone(),
        coroutine_protocol: coroutine_protocol.clone(),
        ffi_protocol: ffi_protocol.clone(),
        foreign_callback_protocol: foreign_callback_protocol.clone(),
        source_location_protocol: source_location_protocol.clone(),
        compiler_operation_protocol: CoreCompilerOperationProtocolV1 {
            operations: Vec::new(),
        },
    };
    let operations = intrinsic_function_kinds()
        .into_iter()
        .map(|kind| {
            let callable = builder.operation(
                fixture_operation_owner(kind, &fundamental_types),
                operation_own_type_parameter_count(kind),
                expected_operation_signature(&signature_surface, kind),
            );
            CoreCompilerOperationV1 {
                kind,
                callable: callable_entry(callable),
            }
        })
        .collect::<Vec<_>>();

    coroutine_protocol.0.entries[11] =
        operation_entry(&operations, IntrinsicFunctionKind::CoroutineStart);
    coroutine_protocol.0.entries[12] =
        operation_entry(&operations, IntrinsicFunctionKind::CoroutineSuspend);
    for (index, kind) in [
        (
            4,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::ToULong),
        ),
        (
            5,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Cast),
        ),
        (
            6,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Load),
        ),
        (
            7,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::LoadOffset),
        ),
        (
            8,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Store),
        ),
        (
            9,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::StoreOffset),
        ),
        (
            10,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Plus),
        ),
        (
            11,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Minus),
        ),
        (
            12,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AddressOf),
        ),
        (
            13,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::SizeOf),
        ),
        (
            14,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AlignOf),
        ),
        (15, IntrinsicFunctionKind::GcPinRaw),
        (16, IntrinsicFunctionKind::GcUnpinRaw),
        (17, IntrinsicFunctionKind::GcGetHandleRaw),
        (18, IntrinsicFunctionKind::GcReleaseHandleRaw),
    ] {
        ffi_protocol.0.entries[index] = operation_entry(&operations, kind);
    }
    for (index, kind) in [
        (10, IntrinsicFunctionKind::ForeignCallbackRegister),
        (11, IntrinsicFunctionKind::ForeignCallbackRetain),
        (12, IntrinsicFunctionKind::ForeignCallbackRelease),
        (13, IntrinsicFunctionKind::ForeignCallbackState),
        (14, IntrinsicFunctionKind::ForeignCallbackFailure),
    ] {
        foreign_callback_protocol.0.entries[index] = operation_entry(&operations, kind);
    }
    source_location_protocol.0.entries[1] =
        operation_entry(&operations, IntrinsicFunctionKind::CurrentSourceLocation);

    let surface = CoreCompilerProtocolSurfaceV1 {
        fundamental_types,
        option_protocol,
        iteration_protocol,
        exception_protocol,
        coroutine_protocol,
        ffi_protocol,
        foreign_callback_protocol,
        source_location_protocol,
        compiler_operation_protocol: CoreCompilerOperationProtocolV1 { operations },
    };
    surface.validate_internal_relations().unwrap();
    builder.install(foundation);
    surface
}

struct EnumFixture {
    owner: PersistentTypeId,
    variants: Vec<PersistentEnumVariantId>,
}

struct FixtureBuilder {
    existing: ExistingProtocolFixture,
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
    fn new(existing: ExistingProtocolFixture) -> Self {
        Self {
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

    fn concrete_type(&mut self) -> PersistentTypeId {
        let record: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(DefinitionOwnerChain::top_level()),
            self.name(),
            SourceNominalKind::Class,
            0,
        ))
        .unwrap();
        let id = record.id();
        self.origins
            .push(origin_record(DefinitionOriginSubject::Type(id)));
        self.types.push(record);
        id
    }

    fn concrete_nominal(&mut self, kind: SourceNominalKind) -> CoreProtocolEntryV1 {
        let record: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(DefinitionOwnerChain::top_level()),
            self.name(),
            kind,
            0,
        ))
        .unwrap();
        let id = record.id();
        self.origins
            .push(origin_record(DefinitionOriginSubject::Type(id)));
        self.types.push(record);
        concrete_entry(id)
    }

    fn generic_nominal(&mut self, kind: SourceNominalKind) -> CoreProtocolEntryV1 {
        let record: GenericTypeRecord =
            CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                site(DefinitionOwnerChain::top_level()),
                self.name(),
                kind,
                1,
            ))
            .unwrap();
        let id = record.id();
        self.origins
            .push(origin_record(DefinitionOriginSubject::GenericType(id)));
        self.generic_types.push(record);
        generic_entry(id)
    }

    fn function(
        &mut self,
        effect: scoop_identity::Effect,
    ) -> (CoreProtocolEntryV1, PersistentFunctionId) {
        let unit = CoreBuiltinNominal::Unit.identity_record().id();
        self.function_with_owner_signature(
            None,
            SignatureCallableShape::new(effect, None, Vec::new(), SignatureTypeKey::Nominal(unit)),
        )
    }

    fn function_with_owner_signature(
        &mut self,
        owner: Option<DefinitionOwnerAtom>,
        signature: SignatureCallableShape,
    ) -> (CoreProtocolEntryV1, PersistentFunctionId) {
        let record: FunctionRecord = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site(owner.map_or_else(DefinitionOwnerChain::top_level, |owner| {
                DefinitionOwnerChain::from_outer_to_inner(vec![owner])
            })),
            self.name(),
            0,
            None,
            signature.parameters().to_vec(),
        ))
        .unwrap();
        let id = record.id();
        self.origins
            .push(origin_record(DefinitionOriginSubject::Function(id)));
        self.functions.push(record);
        (
            CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
                CoreProtocolCallableDefinitionV1::Function(id),
                signature,
            )),
            id,
        )
    }

    fn operation(
        &mut self,
        owner: Option<DefinitionOwnerAtom>,
        own_type_parameter_count: u32,
        signature: SignatureCallableShape,
    ) -> CoreProtocolEntryV1 {
        let source = SourceDeclarationKey::function(
            site(owner.map_or_else(DefinitionOwnerChain::top_level, |owner| {
                DefinitionOwnerChain::from_outer_to_inner(vec![owner])
            })),
            self.name(),
            own_type_parameter_count,
            None,
            signature.parameters().to_vec(),
        );
        if own_type_parameter_count == 0 {
            let record: FunctionRecord = CborIdentityRecord::from_key(source).unwrap();
            let id = record.id();
            self.origins
                .push(origin_record(DefinitionOriginSubject::Function(id)));
            self.functions.push(record);
            CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
                CoreProtocolCallableDefinitionV1::Function(id),
                signature,
            ))
        } else {
            let record: GenericFunctionRecord = CborIdentityRecord::from_key(source).unwrap();
            let id = record.id();
            self.origins
                .push(origin_record(DefinitionOriginSubject::GenericFunction(id)));
            self.generic_functions.push(record);
            CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
                CoreProtocolCallableDefinitionV1::GenericFunction(id),
                signature,
            ))
        }
    }

    fn constructor(
        &mut self,
        owner: PersistentTypeId,
        parameters: Vec<SignatureTypeKey>,
    ) -> CoreProtocolEntryV1 {
        let record: ConstructorRecord =
            CborIdentityRecord::from_key(SourceDeclarationKey::constructor(
                site(DefinitionOwnerChain::from_outer_to_inner(vec![
                    DefinitionOwnerAtom::Type(owner),
                ])),
                parameters.clone(),
            ))
            .unwrap();
        let id = record.id();
        self.origins
            .push(origin_record(DefinitionOriginSubject::Constructor(id)));
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

    fn concrete_enum_with_variants(&mut self, count: usize) -> EnumFixture {
        let record: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(DefinitionOwnerChain::top_level()),
            self.name(),
            SourceNominalKind::Enum,
            0,
        ))
        .unwrap();
        let owner = record.id();
        self.origins
            .push(origin_record(DefinitionOriginSubject::Type(owner)));
        let owner_key = record.key().clone();
        self.types.push(record);
        let variants = (0..count)
            .map(|_| {
                let record: VariantRecord = CborIdentityRecord::from_key(
                    EnumVariantIdentityKey::source(&owner_key, self.name()).unwrap(),
                )
                .unwrap();
                let id = record.id();
                self.origins
                    .push(origin_record(DefinitionOriginSubject::EnumVariant(id)));
                self.variants.push(record);
                id
            })
            .collect();
        EnumFixture { owner, variants }
    }

    fn exact_nominal(&mut self, owner: PersistentTypeId) -> PersistentExactTypeId {
        let record: ExactTypeRecord =
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap();
        let id = record.id();
        self.exact_types.push(record);
        id
    }

    fn exact_application(
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

    fn dispatch(&mut self, function: PersistentFunctionId) -> CoreProtocolEntryV1 {
        let record: DispatchRecord =
            CborIdentityRecord::from_key(DispatchSlotKey::interface_method(function)).unwrap();
        let id = record.id();
        self.dispatch_slots.push(record);
        CoreProtocolEntryV1::DispatchSlot(id)
    }

    fn name(&mut self) -> CanonicalIdentifier {
        let text = format!("protocol{}", self.next_name);
        let name = CanonicalIdentifier::new(&text).unwrap();
        self.next_name += 1;
        name
    }

    fn install(mut self, foundation: &mut CanonicalHirFoundation) {
        self.types.push(self.existing.string.clone());
        self.generic_types.push(self.existing.option.clone());
        self.variants.push(self.existing.option_some.clone());
        self.variants.push(self.existing.option_none.clone());
        self.variant_fields
            .push(self.existing.option_some_payload.clone());
        self.exact_types.append(&mut self.existing.exact_types);
        self.origins
            .push(origin_record(DefinitionOriginSubject::Type(
                self.existing.string.id(),
            )));
        self.origins
            .push(origin_record(DefinitionOriginSubject::GenericType(
                self.existing.option.id(),
            )));
        self.origins
            .push(origin_record(DefinitionOriginSubject::EnumVariant(
                self.existing.option_some.id(),
            )));
        self.origins
            .push(origin_record(DefinitionOriginSubject::EnumVariantField(
                self.existing.option_some_payload.id(),
            )));
        self.origins
            .push(origin_record(DefinitionOriginSubject::EnumVariant(
                self.existing.option_none.id(),
            )));

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

fn product<const N: usize>(entries: [CoreProtocolEntryV1; N]) -> CoreProtocolProductV1<N> {
    CoreProtocolProductV1 { entries }
}

fn concrete_entry(id: PersistentTypeId) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id))
}

fn generic_entry(id: PersistentGenericTypeId) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id))
}

fn signature_application(
    origin: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(argument, []),
    }
}

fn generic_protocol_id(entry: &CoreProtocolEntryV1) -> PersistentGenericTypeId {
    match entry {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => *id,
        _ => unreachable!("the fixture protocol nominal is generic"),
    }
}

fn operation_entry(
    operations: &[CoreCompilerOperationV1],
    kind: IntrinsicFunctionKind,
) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::Callable(
        operations
            .iter()
            .find(|operation| operation.kind == kind)
            .expect("the test fixture constructs every intrinsic operation")
            .callable
            .clone(),
    )
}

fn fixture_operation_owner(
    kind: IntrinsicFunctionKind,
    fundamental: &CoreFundamentalTypeProtocolV1,
) -> Option<DefinitionOwnerAtom> {
    match kind {
        IntrinsicFunctionKind::Integer(kind) => {
            let index = crate::IntegerKind::ALL
                .iter()
                .position(|candidate| *candidate == integer_source_kind(kind))
                .expect("every integer intrinsic uses a canonical integer kind");
            Some(DefinitionOwnerAtom::Type(concrete_entry_ref(
                fundamental.entries(),
                index + 1,
            )))
        }
        IntrinsicFunctionKind::PrimitiveUnary(_) => Some(DefinitionOwnerAtom::Type(
            concrete_entry_ref(fundamental.entries(), 9),
        )),
        IntrinsicFunctionKind::PrimitiveBinary(_) => Some(DefinitionOwnerAtom::Type(
            concrete_entry_ref(fundamental.entries(), 10),
        )),
        IntrinsicFunctionKind::ArrayAccess(kind) => {
            Some(DefinitionOwnerAtom::GenericType(generic_entry_ref(
                fundamental.entries(),
                if kind == crate::ArrayAccessKind::ImmutableGet {
                    11
                } else {
                    12
                },
            )))
        }
        IntrinsicFunctionKind::Array(kind) => {
            Some(DefinitionOwnerAtom::GenericType(generic_entry_ref(
                fundamental.entries(),
                if kind == crate::ArrayIntrinsic::ToImmutable {
                    12
                } else {
                    11
                },
            )))
        }
        IntrinsicFunctionKind::Pointer(
            crate::PointerIntrinsic::ToULong
            | crate::PointerIntrinsic::Cast
            | crate::PointerIntrinsic::Load
            | crate::PointerIntrinsic::LoadOffset
            | crate::PointerIntrinsic::Store
            | crate::PointerIntrinsic::StoreOffset
            | crate::PointerIntrinsic::Plus
            | crate::PointerIntrinsic::Minus,
        ) => Some(DefinitionOwnerAtom::GenericType(generic_entry_ref(
            fundamental.entries(),
            13,
        ))),
        IntrinsicFunctionKind::GcPinRaw
        | IntrinsicFunctionKind::GcUnpinRaw
        | IntrinsicFunctionKind::GcGetHandleRaw
        | IntrinsicFunctionKind::GcReleaseHandleRaw
        | IntrinsicFunctionKind::GcCollect
        | IntrinsicFunctionKind::GcStats
        | IntrinsicFunctionKind::CoroutineStart
        | IntrinsicFunctionKind::CoroutineSuspend
        | IntrinsicFunctionKind::CurrentSourceLocation
        | IntrinsicFunctionKind::ForeignCallbackRegister
        | IntrinsicFunctionKind::ForeignCallbackRetain
        | IntrinsicFunctionKind::ForeignCallbackRelease
        | IntrinsicFunctionKind::ForeignCallbackState
        | IntrinsicFunctionKind::ForeignCallbackFailure
        | IntrinsicFunctionKind::Pointer(_) => None,
    }
}

fn integer_source_kind(kind: crate::IntegerIntrinsicKind) -> crate::IntegerKind {
    match kind {
        crate::IntegerIntrinsicKind::NoGcOperation { kind, .. }
        | crate::IntegerIntrinsicKind::ManagedOperation { kind, .. } => kind,
        crate::IntegerIntrinsicKind::Conversion { source, .. } => source,
    }
}

fn concrete_entry_ref<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id)) => id,
        _ => unreachable!("the fixture fundamental role is a concrete nominal"),
    }
}

fn generic_entry_ref<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentGenericTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("the fixture fundamental role is a generic nominal"),
    }
}

fn site(owners: DefinitionOwnerChain) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        owners,
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn origin_record(subject: DefinitionOriginSubject) -> DefinitionOriginRecord {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/protocol_fixture.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    DefinitionOriginRecord::new(
        subject,
        DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap(),
    )
}
