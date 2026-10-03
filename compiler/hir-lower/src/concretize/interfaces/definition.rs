use super::*;

pub(super) struct ResolvedInterfaceDefinition<'a> {
    pub origin: export::HirNominalIdentity,
    pub name: String,
    pub owner: Option<concrete::NominalOwner>,
    pub parents: &'a [export::TypeId],
    pub members: export::TypeId,
    pub automatic_methods: &'a [export::InterfaceMethodId],
    pub span: scoop_ast::Span,
}

pub(super) struct ResolvedInterfaceMethod<'a> {
    pub slot: scoop_identity::PersistentDispatchSlotId,
    pub overrides: Vec<scoop_identity::PersistentDispatchSlotId>,
    pub name: &'a str,
    pub is_suspend: bool,
    pub attributes: export::FunctionAttributes,
    pub implementation: concrete::InterfaceMemberImplementation,
    pub parameters: Vec<(&'a str, export::TypeId, concrete::LocalId)>,
    pub return_type: export::TypeId,
    pub span: scoop_ast::Span,
}

impl<'a> ResolvedInterfaceMethod<'a> {
    pub(super) fn from_dependency(method: &'a export::LoadedInterfaceMethod) -> Self {
        let effects = method.declaration.effects();
        Self {
            slot: method.slot.id(),
            overrides: method.overrides.clone(),
            name: &method.name,
            is_suspend: effects.execution() == scoop_identity::Effect::Suspend,
            attributes: effects.function_attributes(),
            implementation: match method.declaration.modality() {
                export::CallableModalityV1::Abstract => {
                    concrete::InterfaceMemberImplementation::AbstractSlot
                }
                _ => concrete::InterfaceMemberImplementation::Body,
            },
            parameters: method
                .parameters
                .iter()
                .enumerate()
                .map(|(index, (name, ty))| {
                    (
                        name.as_str(),
                        *ty,
                        concrete::LocalId::from_raw(
                            u32::try_from(index + 1)
                                .expect("method parameter index fits in u32")
                                .into(),
                        ),
                    )
                })
                .collect(),
            return_type: method.return_type,
            span: method.span,
        }
    }
}

impl<'input> Concretizer<'input> {
    pub(super) fn source_interface_method(
        &self,
        member: export::InterfaceMethodId,
    ) -> ResolvedInterfaceMethod<'input> {
        let member_data = &self.source.interface_methods[member];
        let function = &self.source.functions[member_data.function];
        ResolvedInterfaceMethod {
            slot: self.source.dispatch_slot_identities[member].id(),
            overrides: member_data
                .overrides
                .iter()
                .map(|reference| self.interface_reference_slot(*reference))
                .collect(),
            name: function
                .name
                .rsplit('.')
                .next()
                .expect("interface methods have a name"),
            is_suspend: function.is_suspend,
            attributes: function.attributes,
            implementation: match member_data.implementation {
                export::InterfaceMemberImplementation::Body => {
                    concrete::InterfaceMemberImplementation::Body
                }
                export::InterfaceMemberImplementation::AbstractSlot => {
                    concrete::InterfaceMemberImplementation::AbstractSlot
                }
            },
            parameters: function
                .params
                .iter()
                .skip(1)
                .map(|parameter| {
                    (
                        parameter.name.as_str(),
                        parameter.ty,
                        remap_idx(parameter.local),
                    )
                })
                .collect(),
            return_type: function.return_ty,
            span: function.span,
        }
    }
}
