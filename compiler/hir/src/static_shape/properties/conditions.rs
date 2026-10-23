use super::*;

impl<'a> StaticPropertyShape<'a> {
    pub fn availability(
        self,
        world: &ImportedSemanticWorld<'a>,
    ) -> Option<(ConditionalInterfaceId, StaticShapeTypeSource<'a>)> {
        match self.source {
            StaticPropertyDeclaration::Current { declaration, .. } => {
                let module = self.owner.module;
                let getter = &module.property_getters[declaration.capability.getter()];
                let function = match getter.implementation {
                    PropertyAccessorImplementation::Body(function)
                    | PropertyAccessorImplementation::AbstractSlot(function)
                    | PropertyAccessorImplementation::StorageBody(function) => function,
                    PropertyAccessorImplementation::Storage
                    | PropertyAccessorImplementation::Constant => return None,
                };
                let guard = module.functions[function].method?.availability?;
                Some((guard.declaration, StaticShapeTypeSource::Hir(guard.owner)))
            }
            StaticPropertyDeclaration::Dependency(declaration) => {
                let getter = scoop_identity::CallableTemplateOrigin::Accessor(
                    declaration.accessors().getter_source().accessor(),
                );
                let guard = world
                    .nominal_source_provider(self.owner.declaration)
                    .expect("a loaded property retains its provider")
                    .source_interface()
                    .callable_interfaces()
                    .declaration(getter)
                    .expect("a property retains its getter declaration")
                    .availability()?;
                Some((
                    guard.declaration,
                    StaticShapeTypeSource::Published(&guard.owner),
                ))
            }
        }
    }
}
