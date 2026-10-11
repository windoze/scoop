use super::*;
use selection::property_name;

impl Lowerer {
    pub(super) fn declare_delegated_properties(
        &mut self,
        owner: Owner,
        class: ClassId,
        slots: &[DelegateSlot],
    ) -> Vec<FunctionId> {
        let mut functions = Vec::new();
        for getter in slots.iter().filter(|slot| {
            slot.candidate.member.signature.accessor == Some(PropertyAccessorKind::Getter)
        }) {
            let name = property_name(&getter.candidate.member.signature);
            let setter = slots.iter().find(|slot| {
                slot.candidate.member.signature.accessor == Some(PropertyAccessorKind::Setter)
                    && property_name(&slot.candidate.member.signature) == name
            });
            if !std::iter::once(getter).chain(setter).any(|slot| {
                matches!(
                    slot.selection,
                    DelegateSelection::Forward(ForwardingReceiver::Delegate(_))
                )
            }) {
                continue;
            }
            let DelegateSelection::Forward(getter_receiver) = getter.selection else {
                continue;
            };
            let setter_receiver = match setter {
                Some(DelegateSlot {
                    selection: DelegateSelection::Forward(receiver),
                    ..
                }) => Some(*receiver),
                Some(_) => continue,
                None => None,
            };
            let property = self.next_property_id();
            let getter_function = self.allocate_interface_forwarder(
                owner,
                &getter.candidate.member,
                getter_receiver,
                ForwardingRole::Getter(property),
            );
            let access = self.functions[getter_function].access.clone();
            let modifier = self.functions[getter_function]
                .method
                .expect("an accessor is a method")
                .modifier;
            let span = self.functions[getter_function].span;
            let getter_id = self.property_getters.alloc(hir::PropertyGetter {
                access: access.clone(),
                implementation: hir::PropertyAccessorImplementation::Body(getter_function),
                attributes: self.functions[getter_function].attributes,
                span,
            });
            functions.push(getter_function);
            let capability = if let Some((setter, receiver)) = setter.zip(setter_receiver) {
                let function = self.allocate_interface_forwarder(
                    owner,
                    &setter.candidate.member,
                    receiver,
                    ForwardingRole::Setter(property),
                );
                functions.push(function);
                let setter_id = self.property_setters.alloc(hir::PropertySetter {
                    parameter_name: self.signatures[&function].params[0].name.text.clone(),
                    access: self.functions[function].access.clone(),
                    implementation: hir::PropertyAccessorImplementation::Body(function),
                    attributes: self.functions[function].attributes,
                    span: self.functions[function].span,
                });
                hir::PropertyCapability::ReadWrite {
                    getter: getter_id,
                    setter: setter_id,
                }
            } else {
                hir::PropertyCapability::ReadOnly { getter: getter_id }
            };
            let property_owner = match owner {
                Owner::Class(class) => hir::PropertyOwner::Class(class),
                Owner::Object(object) => hir::PropertyOwner::Object(object),
                _ => unreachable!("only reference types delegate interfaces"),
            };
            let actual = self.properties.alloc(hir::Property {
                owner: property_owner,
                name: name.into(),
                access,
                modifier,
                is_override: true,
                overrides: Vec::new(),
                ty: self.signatures[&getter_function].return_ty,
                capability,
                representation: hir::PropertyRepresentation::AccessorOnly,
                span,
            });
            assert_eq!(actual, property);
            self.classes[class].properties.push(property);
            self.imports.bind_property(
                crate::imports::PropertyImportSource::OutsideCurrentUnitSurface,
                property,
            );
        }
        functions
    }
}
