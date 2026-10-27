//! Give COFF definitions independent contributions before machine emission.

use std::collections::BTreeMap;

use inkwell::comdat::ComdatSelectionKind;
use inkwell::module::{Linkage, Module};
use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::core::{LLVMGetFirstGlobalAlias, LLVMGetNextGlobalAlias, LLVMIsAGlobalAlias};
use scoop_lir::{LinkageClass, ObjectSymbolSurfaceV1};

use crate::CodegenError;

pub(crate) fn prepare(
    llvm: &Module<'_>,
    surface: &ObjectSymbolSurfaceV1,
) -> Result<(), CodegenError> {
    let mut values = BTreeMap::new();
    for value in llvm
        .get_globals()
        .chain(llvm.get_functions().map(|value| value.as_global_value()))
    {
        values.insert(value.get_name().to_bytes().to_vec(), value);
    }
    let mut alias = unsafe { LLVMGetFirstGlobalAlias(llvm.as_mut_ptr()) };
    while !alias.is_null() {
        let value = unsafe { GlobalValue::new(alias) };
        values.insert(value.get_name().to_bytes().to_vec(), value);
        alias = unsafe { LLVMGetNextGlobalAlias(alias) };
    }
    let primary_groups = surface
        .plans()
        .iter()
        .map(|plan| {
            (
                plan.primary_symbol().symbol().to_string(),
                plan.primary_symbol(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut owners = BTreeMap::new();
    for plan in surface.plans() {
        for request in std::iter::once(plan.primary_symbol()).chain(
            plan.atom_boundaries()
                .iter()
                .flat_map(|boundary| [boundary.start(), boundary.end()]),
        ) {
            let Some(&value) = values.get(request.symbol().as_str().as_bytes()) else {
                continue;
            };
            if value.is_declaration() {
                continue;
            }
            let owner = crate::llvm_definitions::definition_owner(value)?;
            let primary = owner
                .get_name()
                .to_str()
                .ok()
                .and_then(|name| primary_groups.get(name))
                .copied()
                .unwrap_or(plan.primary_symbol());
            let group = if values
                .get(primary.symbol().as_str().as_bytes())
                .is_some_and(|value| !value.is_declaration())
            {
                primary.symbol().to_string()
            } else {
                // A function-only module can contain image-owned constants
                // without the image descriptor itself. Use an emitted boundary
                // as the section key until native contribution association.
                owners
                    .get(&(owner.as_value_ref() as usize))
                    .cloned()
                    .unwrap_or_else(|| request.symbol().to_string())
            };
            if let Some(previous) = owners.insert(owner.as_value_ref() as usize, group.clone())
                && previous != group
            {
                return Err(CodegenError(format!(
                    "COFF storage belongs to both {previous} and {group}"
                )));
            }
            // COFF aliases must be real section definitions. Their section's
            // COMDAT selection carries ODR semantics, not weak-external fallback.
            if unsafe { !LLVMIsAGlobalAlias(value.as_value_ref()).is_null() } {
                value.set_linkage(Linkage::External);
            }
            let comdat = llvm.get_or_insert_comdat(&group);
            comdat.set_selection_kind(match primary.linkage() {
                LinkageClass::OdrWeak => ComdatSelectionKind::Any,
                LinkageClass::ConeStrong | LinkageClass::TemplateSupportHidden => {
                    ComdatSelectionKind::NoDuplicates
                }
                LinkageClass::RuntimeAbi => {
                    return Err(CodegenError(
                        "runtime declaration cannot own compiler storage".into(),
                    ));
                }
            });
            owner.set_comdat(comdat);
        }
    }
    Ok(())
}
