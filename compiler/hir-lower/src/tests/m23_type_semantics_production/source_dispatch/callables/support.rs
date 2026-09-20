use super::*;

pub(super) fn verify_concrete(output: &hir::OrdinaryHirOutput<'_>, table: &Table) {
    let local = output.output().local.module();
    let export = output.output().export.module();
    for record in table.records() {
        let exact = |ty| local.exact_type_identities[ty].id();
        let (owner, parameters, result, suspend) = if let Some((_, function)) =
            local.functions.iter().find(|(_, function)| {
                function.materialization.template() == template(record.declaration())
            }) {
            (
                function.receiver.method().unwrap().owner,
                &function.params[1..],
                function.return_ty,
                function.is_suspend,
            )
        } else {
            let receiver = record
                .signature()
                .exact_signature()
                .receiver()
                .into_option()
                .unwrap();
            let (interface, definition) = local
                .interfaces
                .iter()
                .find(|(_, definition)| exact(definition.canonical_type) == receiver)
                .unwrap();
            let (slot, _) = local
                .dispatch_slot_identities
                .interface_slots(interface)
                .find(|(_, slot)| {
                    use scoop_identity::{DispatchDeclarationOwner as Owner, DispatchRole as Role};
                    match (slot.key().owner(), slot.key().role(), record.declaration()) {
                        (Owner::Function(a), Role::InterfaceMethod, Declaration::Function(b)) => {
                            a == b
                        }
                        (Owner::Accessor(a), Role::PropertyGetter, Declaration::Getter(b)) => {
                            a == b
                        }
                        (Owner::Accessor(a), Role::PropertySetter, Declaration::Setter(b)) => {
                            a == b
                        }
                        _ => false,
                    }
                })
                .unwrap();
            let method = &definition.methods[slot.into_raw() as usize];
            assert_eq!(
                record.modality(),
                match method.implementation {
                    hir::concrete::InterfaceMemberImplementation::Body =>
                        hir::CallableModalityV1::InterfaceDefault,
                    hir::concrete::InterfaceMemberImplementation::AbstractSlot =>
                        hir::CallableModalityV1::Abstract,
                }
            );
            (
                definition.canonical_type,
                method.params.as_slice(),
                method.return_ty,
                method.is_suspend,
            )
        };
        assert_eq!(
            record
                .signature()
                .exact_signature()
                .receiver()
                .into_option(),
            Some(exact(owner))
        );
        assert_eq!(
            record.signature().exact_signature().parameters(),
            parameters
                .iter()
                .map(|parameter| exact(parameter.ty))
                .collect::<Vec<_>>()
        );
        assert_eq!(record.signature().exact_signature().result(), exact(result));
        assert_eq!(
            record.signature().effects().execution(),
            if suspend {
                scoop_identity::Effect::Suspend
            } else {
                scoop_identity::Effect::Ordinary
            }
        );
        let subject = match record.declaration() {
            Declaration::Function(id) => DefinitionOriginSubject::Function(id),
            Declaration::Getter(id) | Declaration::Setter(id) => {
                DefinitionOriginSubject::PropertyAccessor(id)
            }
        };
        assert_eq!(
            record.declaration_access().definition_origin().origin(),
            export
                .export_definition_origins
                .get(subject)
                .unwrap()
                .origin()
        );
        assert_eq!(record.facts().signature, record.signature());
        assert_eq!(record.facts().modality, record.modality());
        assert_eq!(
            record.facts().declaration_access,
            record.declaration_access()
        );
    }
}

pub(super) fn render(output: &hir::OrdinaryHirOutput<'_>, table: &Table) -> String {
    let mut lines = table
        .records()
        .iter()
        .map(|record| {
            let effects = record.signature().effects();
            format!(
                "{}: {} params, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}\n",
                source_name(output, record.declaration()),
                record.signature().exact_signature().parameters().len(),
                record.modality(),
                record.declaration_access().declared_visibility(),
                effects.execution(),
                effects.safety(),
                effects.gc_effect(),
                effects.operator_role(),
                effects.infix()
            )
        })
        .collect::<Vec<_>>();
    lines.sort();
    lines.concat()
}

fn source_name<'a>(output: &'a hir::OrdinaryHirOutput<'_>, declaration: Declaration) -> &'a str {
    let export = output.output().export.module();
    export
        .functions
        .iter()
        .find_map(|(id, function)| {
            let actual = match &export.function_identities[id] {
                hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                    Declaration::Function(record.id())
                }
                hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Getter(id),
                ) => Declaration::Getter(export.property_accessor_identities[*id].id()),
                hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Setter(id),
                ) => Declaration::Setter(export.property_accessor_identities[*id].id()),
                _ => return None,
            };
            (actual == declaration).then_some(function.name.as_str())
        })
        .unwrap()
}
