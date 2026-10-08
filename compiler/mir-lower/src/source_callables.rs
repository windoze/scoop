//! Persistent LocalConcrete callable locations retained by MIR lowering.

use scoop_hir::concrete as hir;
use scoop_mir as mir;

use super::materialization_odr_group;

#[derive(Default)]
pub(super) struct SourceCallableRegistry {
    entries: Vec<mir::SourceCallableMaterialization>,
}

impl SourceCallableRegistry {
    pub(super) fn record_function(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        source: hir::FunctionId,
    ) {
        let declaration = &module.functions[source];
        let signature = exact_function_signature(module, source);
        if let hir::CallableTemplateOwner::Generated(generated) =
            declaration.materialization.template()
            && module.generated_callable_identities.iter().any(|record| {
                record.id() == generated
                    && matches!(
                        record.key(),
                        hir::GeneratedCallableKey::DerivedEquality { .. }
                    )
            })
        {
            let owner = declaration
                .receiver
                .value_type()
                .expect("derived equality has its exact owner as receiver");
            let entry = mir::SourceCallableMaterialization::derived_equality(
                function,
                &module.exact_type_identities[owner],
                module.exact_type_identities.nominal_specialization(owner),
                signature,
            )
            .expect("derived equality retains its exact type ownership");
            debug_assert_eq!(entry.materialization(), declaration.materialization);
            self.entries.push(entry);
            return;
        }
        self.record(module, function, declaration.materialization, signature);
    }

    pub(super) fn record_class_constructor(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        source: hir::ClassConstructorId,
    ) {
        let constructor = &module.class_constructors[source];
        let signature = exact_class_initializer_signature(module, source);
        self.record(module, function, constructor.materialization, signature);
    }

    pub(super) fn record_struct_constructor(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        source: hir::StructConstructorId,
    ) {
        let constructor = &module.struct_constructors[source];
        let signature = exact_struct_constructor_signature(module, source);
        self.record(module, function, constructor.materialization, signature);
    }

    pub(super) fn record_callable_reference(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        source: hir::CallableReferenceId,
    ) {
        let reference = &module.callable_references[source];
        let signature = exact_function_type_signature(module, reference.function_type);
        self.record(
            module,
            function,
            *reference.identity.materialization(),
            signature,
        );
    }

    fn record(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        materialization: hir::CallableMaterialization,
        signature: hir::ExactCallableSignature,
    ) {
        self.entries.push(
            mir::SourceCallableMaterialization::new(
                function,
                materialization,
                signature,
                materialization_odr_group(module, materialization),
            )
            .expect("a LocalConcrete callable has one valid implementation subject"),
        );
    }

    pub(super) fn required_types(&self, module: &hir::Module) -> Vec<hir::TypeId> {
        let mut types = self
            .entries
            .iter()
            .flat_map(|entry| {
                let signature = entry.signature_record().signature();
                signature
                    .receiver()
                    .into_option()
                    .into_iter()
                    .chain(signature.parameters().iter().copied())
                    .chain(std::iter::once(signature.result()))
            })
            .map(|identity| {
                module
                    .exact_type_identities
                    .type_for_identity(identity)
                    .expect("a source callable signature only references LocalConcrete types")
            })
            .collect::<Vec<_>>();
        types.sort_by_key(|ty| ty.into_raw().into_u32());
        types.dedup();
        types
    }

    pub(super) fn finish(self) -> mir::SourceCallableMaterializations {
        mir::SourceCallableMaterializations::checked(self.entries)
            .expect("MIR lowering records every source callable exactly once")
    }
}

pub(super) fn exact_function_type_signature(
    module: &hir::Module,
    function_type: hir::FunctionTypeId,
) -> hir::ExactCallableSignature {
    let function_type = &module.function_types[function_type];
    hir::ExactCallableSignature::new(
        if function_type.is_suspend {
            hir::Effect::Suspend
        } else {
            hir::Effect::Ordinary
        },
        None,
        function_type
            .parameter_types
            .iter()
            .map(|parameter| module.exact_type_identities[*parameter].id())
            .collect(),
        module.exact_type_identities[function_type.return_type].id(),
    )
}

pub(super) fn exact_function_signature(
    module: &hir::Module,
    function: hir::FunctionId,
) -> hir::ExactCallableSignature {
    let declaration = &module.functions[function];
    if let hir::FunctionKind::Extern(external) = declaration.kind {
        let external = &module.extern_functions[external];
        return hir::ExactCallableSignature::new(
            hir::Effect::Ordinary,
            None,
            external
                .params
                .iter()
                .map(|ty| module.exact_type_identities[*ty].id())
                .collect(),
            module.exact_type_identities[*external.result.scoop_type()].id(),
        );
    }
    let receiver = declaration.receiver.value_type();
    let parameters = if let Some(receiver_type) = receiver {
        let (receiver, parameters) = declaration
            .params
            .split_first()
            .expect("a LocalConcrete receiver has one physical receiver parameter");
        assert_eq!(
            receiver.ty, receiver_type,
            "a LocalConcrete receiver parameter matches its exact receiver type",
        );
        parameters
    } else {
        declaration.params.as_slice()
    };
    hir::ExactCallableSignature::new(
        if declaration.is_suspend {
            hir::Effect::Suspend
        } else {
            hir::Effect::Ordinary
        },
        receiver.map(|receiver| module.exact_type_identities[receiver].id()),
        parameters
            .iter()
            .map(|parameter| module.exact_type_identities[parameter.ty].id())
            .collect(),
        module.exact_type_identities[declaration.return_ty].id(),
    )
}

pub(super) fn exact_class_initializer_signature(
    module: &hir::Module,
    source: hir::ClassConstructorId,
) -> hir::ExactCallableSignature {
    let constructor = &module.class_constructors[source];
    hir::ExactCallableSignature::new(
        hir::Effect::Ordinary,
        Some(exact_class(module, constructor.class)),
        constructor
            .parameters
            .iter()
            .map(|parameter| module.exact_type_identities[parameter.ty].id())
            .collect(),
        module.exact_type_identities[module.unit].id(),
    )
}

pub(super) fn exact_struct_constructor_signature(
    module: &hir::Module,
    source: hir::StructConstructorId,
) -> hir::ExactCallableSignature {
    let constructor = &module.struct_constructors[source];
    hir::ExactCallableSignature::new(
        hir::Effect::Ordinary,
        None,
        constructor
            .parameters
            .iter()
            .map(|parameter| module.exact_type_identities[parameter.ty].id())
            .collect(),
        exact_struct(module, constructor.structure),
    )
}

fn exact_class(module: &hir::Module, class: hir::ClassId) -> hir::PersistentExactTypeId {
    module
        .types
        .iter()
        .find_map(|(ty, declaration)| {
            matches!(&declaration.kind, hir::TypeKind::Class(found) if *found == class)
                .then(|| module.exact_type_identities[ty].id())
        })
        .expect("a concrete class has one canonical exact type")
}

fn exact_struct(module: &hir::Module, structure: hir::StructId) -> hir::PersistentExactTypeId {
    module
        .types
        .iter()
        .find_map(|(ty, declaration)| {
            matches!(&declaration.kind, hir::TypeKind::Struct(found) if *found == structure)
                .then(|| module.exact_type_identities[ty].id())
        })
        .expect("a concrete struct has one canonical exact type")
}
