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
        let signature = hir::ExactCallableSignature::new(
            if declaration.is_suspend {
                hir::Effect::Suspend
            } else {
                hir::Effect::Ordinary
            },
            declaration
                .method
                .map(|method| module.exact_type_identities[method.owner].id()),
            declaration
                .params
                .iter()
                .map(|parameter| module.exact_type_identities[parameter.ty].id())
                .collect(),
            module.exact_type_identities[declaration.return_ty].id(),
        );
        self.record(module, function, declaration.materialization, signature);
    }

    pub(super) fn record_class_constructor(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        source: hir::ClassConstructorId,
    ) {
        let constructor = &module.class_constructors[source];
        let signature = hir::ExactCallableSignature::new(
            hir::Effect::Ordinary,
            Some(exact_class(module, constructor.class)),
            constructor
                .parameters
                .iter()
                .map(|parameter| module.exact_type_identities[parameter.ty].id())
                .collect(),
            module.exact_type_identities[module.unit].id(),
        );
        self.record(module, function, constructor.materialization, signature);
    }

    pub(super) fn record_struct_constructor(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        source: hir::StructConstructorId,
    ) {
        let constructor = &module.struct_constructors[source];
        let signature = hir::ExactCallableSignature::new(
            hir::Effect::Ordinary,
            None,
            constructor
                .parameters
                .iter()
                .map(|parameter| module.exact_type_identities[parameter.ty].id())
                .collect(),
            exact_struct(module, constructor.structure),
        );
        self.record(module, function, constructor.materialization, signature);
    }

    pub(super) fn record_callable_reference(
        &mut self,
        module: &hir::Module,
        function: mir::FunctionId,
        source: hir::CallableReferenceId,
    ) {
        let reference = &module.callable_references[source];
        let function_type = &module.function_types[reference.function_type];
        let signature = hir::ExactCallableSignature::new(
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
        );
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
