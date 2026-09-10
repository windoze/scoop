//! Persistent dispatch-slot identities for export HIR.

use std::collections::{BTreeMap, HashSet};
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{CborIdentityRecord, DispatchRole, DispatchSlotKey, PersistentDispatchSlotId};

use crate::{
    Function, FunctionId, HirFunctionIdentities, HirFunctionIdentity, HirPropertyAccessorFunction,
    HirPropertyAccessorIdentities, HirSourceFunctionIdentity, InterfaceMemberRole, InterfaceMethod,
    InterfaceMethodId, MethodDispatch, VirtualMethodId,
};

mod error;
pub use error::HirDispatchSlotIdentityError;

pub type HirDispatchSlotIdentity = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;

#[derive(Clone, Debug)]
struct HirVirtualDispatchSlotIdentity {
    root: FunctionId,
    record: HirDispatchSlotIdentity,
}

pub struct HirDispatchSlotIdentityInputs<'a> {
    pub functions: &'a Arena<Function>,
    pub function_identities: &'a HirFunctionIdentities,
    pub property_accessor_identities: &'a HirPropertyAccessorIdentities,
    pub interface_methods: &'a Arena<InterfaceMethod>,
}

/// Total slot relation for every dispatch family in an export HIR module.
#[derive(Clone, Debug)]
pub struct HirDispatchSlotIdentities {
    virtual_slots: BTreeMap<VirtualMethodId, HirVirtualDispatchSlotIdentity>,
    interface_slots: Vec<HirDispatchSlotIdentity>,
}

impl HirDispatchSlotIdentities {
    pub fn checked(
        inputs: HirDispatchSlotIdentityInputs<'_>,
        virtual_slots: Vec<(VirtualMethodId, FunctionId, HirDispatchSlotIdentity)>,
        interface_slots: Vec<HirDispatchSlotIdentity>,
    ) -> Result<Self, HirDispatchSlotIdentityError> {
        if interface_slots.len() != inputs.interface_methods.len() {
            return Err(HirDispatchSlotIdentityError::InterfaceLength {
                expected: inputs.interface_methods.len(),
                actual: interface_slots.len(),
            });
        }

        let mut virtual_map = BTreeMap::new();
        for (family, root, record) in virtual_slots {
            if virtual_map
                .insert(family, HirVirtualDispatchSlotIdentity { root, record })
                .is_some()
            {
                return Err(HirDispatchSlotIdentityError::DuplicateVirtualFamily {
                    family: family.into_raw(),
                });
            }
        }

        validate_virtual_slots(&inputs, &virtual_map)?;
        validate_interface_slots(&inputs, &interface_slots)?;
        validate_unique_persistent_ids(&virtual_map, &interface_slots)?;

        Ok(Self {
            virtual_slots: virtual_map,
            interface_slots,
        })
    }

    pub fn get_virtual(&self, family: VirtualMethodId) -> Option<&HirDispatchSlotIdentity> {
        self.virtual_slots.get(&family).map(|slot| &slot.record)
    }

    pub fn virtual_root(&self, family: VirtualMethodId) -> Option<FunctionId> {
        self.virtual_slots.get(&family).map(|slot| slot.root)
    }

    pub fn get_interface(&self, member: InterfaceMethodId) -> Option<&HirDispatchSlotIdentity> {
        self.interface_slots.get(local_index(member))
    }
}

impl Index<VirtualMethodId> for HirDispatchSlotIdentities {
    type Output = HirDispatchSlotIdentity;

    fn index(&self, family: VirtualMethodId) -> &Self::Output {
        self.get_virtual(family)
            .expect("validated virtual dispatch family has an identity")
    }
}

impl Index<InterfaceMethodId> for HirDispatchSlotIdentities {
    type Output = HirDispatchSlotIdentity;

    fn index(&self, member: InterfaceMethodId) -> &Self::Output {
        &self.interface_slots[local_index(member)]
    }
}

fn validate_virtual_slots(
    inputs: &HirDispatchSlotIdentityInputs<'_>,
    slots: &BTreeMap<VirtualMethodId, HirVirtualDispatchSlotIdentity>,
) -> Result<(), HirDispatchSlotIdentityError> {
    let mut referenced = HashSet::new();
    for (function, declaration) in inputs.functions.iter() {
        let Some(method) = declaration.method else {
            continue;
        };
        let family = match method.dispatch {
            MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => family,
            MethodDispatch::Direct | MethodDispatch::Interface(_) => continue,
        };
        let slot =
            slots
                .get(&family)
                .ok_or(HirDispatchSlotIdentityError::UnknownVirtualFamily {
                    function: raw_index(function),
                    family: family.into_raw(),
                })?;
        referenced.insert(family);
        if function_slot_role(inputs, function)? != slot.record.key().role() {
            return Err(HirDispatchSlotIdentityError::VirtualMemberRole {
                function: raw_index(function),
                family: family.into_raw(),
            });
        }
    }

    for (family, slot) in slots {
        if local_index(slot.root) >= inputs.functions.len() {
            return Err(HirDispatchSlotIdentityError::UnknownVirtualRoot {
                family: family.into_raw(),
                function: raw_index(slot.root),
            });
        }
        let root = &inputs.functions[slot.root];
        if !matches!(
            root.method.map(|method| method.dispatch),
            Some(MethodDispatch::Virtual(actual)) if actual == *family
        ) {
            return Err(HirDispatchSlotIdentityError::VirtualRootDispatch {
                family: family.into_raw(),
                function: raw_index(slot.root),
            });
        }
        let expected = dispatch_key(inputs, slot.root, DispatchDeclarationKind::Virtual)?;
        if slot.record.key() != &expected {
            return Err(HirDispatchSlotIdentityError::VirtualIdentity {
                family: family.into_raw(),
            });
        }
        if !referenced.contains(family) {
            return Err(HirDispatchSlotIdentityError::UnreferencedVirtualFamily {
                family: family.into_raw(),
            });
        }
    }
    Ok(())
}

fn validate_interface_slots(
    inputs: &HirDispatchSlotIdentityInputs<'_>,
    slots: &[HirDispatchSlotIdentity],
) -> Result<(), HirDispatchSlotIdentityError> {
    for (member, declaration) in inputs.interface_methods.iter() {
        if local_index(declaration.function) >= inputs.functions.len() {
            return Err(HirDispatchSlotIdentityError::UnknownInterfaceFunction {
                member: raw_index(member),
                function: raw_index(declaration.function),
            });
        }
        let function = &inputs.functions[declaration.function];
        if !matches!(
            function.method.map(|method| method.dispatch),
            Some(MethodDispatch::Interface(actual)) if actual == member
        ) {
            return Err(HirDispatchSlotIdentityError::InterfaceDispatch {
                member: raw_index(member),
                function: raw_index(declaration.function),
            });
        }
        validate_interface_role(inputs, member, declaration)?;
        let expected = dispatch_key(
            inputs,
            declaration.function,
            DispatchDeclarationKind::Interface,
        )?;
        if slots[local_index(member)].key() != &expected {
            return Err(HirDispatchSlotIdentityError::InterfaceIdentity {
                member: raw_index(member),
            });
        }
    }
    Ok(())
}

fn validate_interface_role(
    inputs: &HirDispatchSlotIdentityInputs<'_>,
    member: InterfaceMethodId,
    declaration: &InterfaceMethod,
) -> Result<(), HirDispatchSlotIdentityError> {
    let identity = &inputs.function_identities[declaration.function];
    let valid = match (declaration.role, identity) {
        (
            InterfaceMemberRole::Function,
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(_)),
        ) => true,
        (
            InterfaceMemberRole::PropertyGetter(property),
            HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Getter(getter)),
        ) => inputs
            .property_accessor_identities
            .get_getter(*getter)
            .is_some_and(|identity| identity.property() == property),
        (
            InterfaceMemberRole::PropertySetter(property),
            HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Setter(setter)),
        ) => inputs
            .property_accessor_identities
            .get_setter(*setter)
            .is_some_and(|identity| identity.property() == property),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(HirDispatchSlotIdentityError::InterfaceRole {
            member: raw_index(member),
            function: raw_index(declaration.function),
        })
    }
}

#[derive(Clone, Copy)]
enum DispatchDeclarationKind {
    Virtual,
    Interface,
}

fn dispatch_key(
    inputs: &HirDispatchSlotIdentityInputs<'_>,
    function: FunctionId,
    kind: DispatchDeclarationKind,
) -> Result<DispatchSlotKey, HirDispatchSlotIdentityError> {
    match &inputs.function_identities[function] {
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => Ok(match kind {
            DispatchDeclarationKind::Virtual => DispatchSlotKey::virtual_method(record.id()),
            DispatchDeclarationKind::Interface => DispatchSlotKey::interface_method(record.id()),
        }),
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Getter(getter)) => {
            let accessor = inputs
                .property_accessor_identities
                .get_getter(*getter)
                .ok_or(HirDispatchSlotIdentityError::InvalidDeclarationIdentity {
                    function: raw_index(function),
                })?;
            Ok(DispatchSlotKey::property_getter(accessor.id()))
        }
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Setter(setter)) => {
            let accessor = inputs
                .property_accessor_identities
                .get_setter(*setter)
                .ok_or(HirDispatchSlotIdentityError::InvalidDeclarationIdentity {
                    function: raw_index(function),
                })?;
            Ok(DispatchSlotKey::property_setter(accessor.id()))
        }
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(_))
        | HirFunctionIdentity::LexicalGenerated(_)
        | HirFunctionIdentity::Initialization { .. }
        | HirFunctionIdentity::DerivedEquality(_) => {
            Err(HirDispatchSlotIdentityError::InvalidDeclarationIdentity {
                function: raw_index(function),
            })
        }
    }
}

fn function_slot_role(
    inputs: &HirDispatchSlotIdentityInputs<'_>,
    function: FunctionId,
) -> Result<DispatchRole, HirDispatchSlotIdentityError> {
    match &inputs.function_identities[function] {
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(_)) => {
            Ok(DispatchRole::VirtualMethod)
        }
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Getter(_)) => {
            Ok(DispatchRole::PropertyGetter)
        }
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Setter(_)) => {
            Ok(DispatchRole::PropertySetter)
        }
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(_))
        | HirFunctionIdentity::LexicalGenerated(_)
        | HirFunctionIdentity::Initialization { .. }
        | HirFunctionIdentity::DerivedEquality(_) => {
            Err(HirDispatchSlotIdentityError::InvalidDeclarationIdentity {
                function: raw_index(function),
            })
        }
    }
}

fn validate_unique_persistent_ids(
    virtual_slots: &BTreeMap<VirtualMethodId, HirVirtualDispatchSlotIdentity>,
    interface_slots: &[HirDispatchSlotIdentity],
) -> Result<(), HirDispatchSlotIdentityError> {
    let mut seen = HashSet::new();
    for record in virtual_slots
        .values()
        .map(|slot| &slot.record)
        .chain(interface_slots)
    {
        if !seen.insert(record.id()) {
            return Err(HirDispatchSlotIdentityError::DuplicatePersistentIdentity {
                identity: record.id(),
            });
        }
    }
    Ok(())
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
