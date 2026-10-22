use super::*;
use hir::ImportedCallableSource;
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId};

impl BodyQueries<'_> {
    pub(super) fn imported_constructor(
        &mut self,
        id: PersistentConstructorId,
    ) -> Option<Availability<Arc<hir::SemanticConstructorBody>>> {
        let dependencies = self.lowerer.dependencies.as_ref()?;
        let declaration = dependencies
            .callable_declaration(CallableTemplateOrigin::Constructor(id))
            .ok()?;
        let interface = declaration.interface();
        let owner = interface
            .owner()
            .nominal_owner()
            .expect("a constructor has its checked nominal owner");
        let nominal = dependencies
            .nominal_declaration(owner)
            .expect("a checked constructor retains its nominal declaration");
        if matches!(
            nominal.interface.source_shape(),
            hir::NominalSourceShapeV1::Intrinsic(_)
        ) || interface.effects().implementation() != hir::CallableImplementationV1::Scoop
        {
            return Some(Availability::NoBody);
        }
        let hir::SourceNominalId::GenericTemplate(owner) = owner else {
            return Some(Availability::NotPublished);
        };
        let Some(initialization) = dependencies.nominal_initialization(owner) else {
            return Some(Availability::NotPublished);
        };
        let initialization = initialization.initialization();
        let Some(declaration) = initialization.constructors().iter().find(|constructor| {
            match constructor.declaration() {
                hir::DefaultConstructorRefV1::Struct { declaration, .. } => *declaration == id,
                hir::DefaultConstructorRefV1::Class {
                    declaration: hir::DefaultClassConstructorIdV1::Source(declaration),
                    ..
                } => *declaration == id,
                hir::DefaultConstructorRefV1::Class {
                    declaration: hir::DefaultClassConstructorIdV1::Generated(_),
                    ..
                }
                | hir::DefaultConstructorRefV1::Variant { .. } => false,
            }
        }) else {
            return Some(Availability::NotPublished);
        };
        let common = Arc::clone(
            self.imported_common
                .entry(owner)
                .or_insert_with(|| initialization.common().to_vec().into()),
        );
        Some(Availability::Available(Arc::new(
            hir::SemanticConstructorBody {
                declaration: Arc::new(declaration.clone()),
                common,
            },
        )))
    }
}
