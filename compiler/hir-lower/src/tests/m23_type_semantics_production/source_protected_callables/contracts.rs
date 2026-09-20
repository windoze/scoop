use super::*;

fn identities(
    export: &hir::ExportHir,
) -> BTreeMap<CallableTemplateOrigin, (String, DefinitionOriginSubject)> {
    let mut result = BTreeMap::new();
    for (id, function) in export.functions.iter() {
        let (declaration, subject) = match &export.function_identities[id] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => (
                CallableTemplateOrigin::Function(record.id()),
                DefinitionOriginSubject::Function(record.id()),
            ),
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => (
                CallableTemplateOrigin::GenericFunction(record.id()),
                DefinitionOriginSubject::GenericFunction(record.id()),
            ),
            _ => continue,
        };
        result.insert(declaration, (function.name.clone(), subject));
    }
    for (_, property) in export.properties.iter() {
        let hir::PropertyOwner::Class(class) = property.owner else {
            continue;
        };
        let prefix = format!("{}.{}", export.classes[class].name, property.name);
        let getter = export.property_accessor_identities[property.capability.getter()].id();
        result.insert(
            CallableTemplateOrigin::Accessor(getter),
            (
                format!("{prefix}.get"),
                DefinitionOriginSubject::PropertyAccessor(getter),
            ),
        );
        if let Some(setter) = property.capability.setter() {
            let setter = export.property_accessor_identities[setter].id();
            result.insert(
                CallableTemplateOrigin::Accessor(setter),
                (
                    format!("{prefix}.set"),
                    DefinitionOriginSubject::PropertyAccessor(setter),
                ),
            );
        }
    }
    result
}

pub(super) fn verify(output: &hir::OrdinaryHirOutput<'_>, table: &Table) {
    let export = output.output().export.module();
    let identities = identities(export);
    for record in table.records() {
        let (_, subject) = &identities[&record.declaration()];
        let access = record.declaration_access();
        assert_eq!(
            access.declared_visibility(),
            hir::DeclaredVisibilityV1::Protected
        );
        assert_eq!(
            access.definition_origin().origin(),
            export
                .export_definition_origins
                .get(*subject)
                .unwrap()
                .origin()
        );
        assert_eq!(
            access.lexical_owners().last(),
            Some(&record.payload().owner())
        );
        assert!(!record.payload().receiver().is_present());
    }
    for (id, function) in export.functions.iter() {
        let hir::HirFunctionIdentity::Source(identity) = &export.function_identities[id] else {
            continue;
        };
        let declaration = match identity {
            hir::HirSourceFunctionIdentity::Plain(record) => {
                CallableTemplateOrigin::Function(record.id())
            }
            hir::HirSourceFunctionIdentity::Generic(record) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
        };
        let Some(record) = table.get(declaration) else {
            continue;
        };
        let payload = record.payload();
        let scoop_identity::DuplicateSignatureKey::Function { parameters, .. } =
            identity.declaration().duplicate_signature()
        else {
            panic!("function source key");
        };
        assert_eq!(
            payload
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| parameter.value_type())
                .collect::<Vec<_>>(),
            parameters.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            payload.effects().safety() == hir::CallableSafetyV1::Unsafe,
            function.attributes.safety == hir::Safety::Unsafe
        );
        assert_eq!(
            payload.effects().execution() == scoop_identity::Effect::Suspend,
            function.is_suspend
        );
        let expected: Vec<_> = match function.method.unwrap().dispatch {
            hir::MethodDispatch::Direct => Vec::new(),
            hir::MethodDispatch::Virtual(family) | hir::MethodDispatch::FinalOverride(family) => {
                vec![export.dispatch_slot_identities[family].id()]
            }
            hir::MethodDispatch::Interface(_) => panic!("protected class declaration"),
        };
        assert_eq!(payload.slot_relations().slots(), expected);
    }
}

pub(super) fn render(output: &hir::OrdinaryHirOutput<'_>, table: &Table) -> String {
    let identities = identities(output.output().export.module());
    let mut lines = table
        .records()
        .iter()
        .map(|record| {
            let payload = record.payload();
            format!(
                "{}({}): {:?}, {:?}, {:?}, binders={}, slots={}\n",
                identities[&record.declaration()].0,
                payload
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.name().as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                payload.modality(),
                payload.effects().safety(),
                payload.effects().execution(),
                payload.type_parameters().len_u32(),
                payload.slot_relations().slots().len()
            )
        })
        .collect::<Vec<_>>();
    lines.sort();
    lines.concat()
}
