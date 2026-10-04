//! Callable-associated Context key tables and writable slot cells.

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::PointerValue;
use scoop_lir::{
    ObjectSymbolSurfaceV1, StrongCallableRegistrationPlanV1, StrongCallableRuntimeScanPlanV1,
};

use crate::atom_boundaries::GlobalAtomMaterializationV1;
use crate::{CodegenError, ptr_ty};

pub(super) fn emit<'ctx>(
    context: &'ctx Context,
    llvm: &Module<'ctx>,
    surface: &ObjectSymbolSurfaceV1,
    profile: crate::target::ValidatedBackendProfile,
    registration: StrongCallableRegistrationPlanV1,
    plan: &StrongCallableRuntimeScanPlanV1,
    atoms: &mut Vec<GlobalAtomMaterializationV1<'ctx>>,
) -> Result<PointerValue<'ctx>, CodegenError> {
    if plan.context_keys().is_empty() {
        return Ok(ptr_ty(context).const_null());
    }
    let symbols = surface
        .plan(registration.body_definition_plan())
        .ok_or_else(|| {
            CodegenError(format!(
                "Context key table has no callable definition {}",
                plan.body()
            ))
        })?;
    let boundary = |atom| {
        symbols
            .atom_boundaries()
            .iter()
            .find(|boundary| boundary.atom() == atom)
            .copied()
            .ok_or_else(|| CodegenError(format!("Context key atom {atom} has no boundary")))
    };
    let i64 = context.i64_type();
    let digest = context.struct_type(&[context.i8_type().array_type(32).into()], false);
    let entry = context.struct_type(&[digest.into(), ptr_ty(context).into()], false);
    let mut entries = Vec::with_capacity(plan.context_keys().len());
    for cell in plan.context_keys() {
        let symbol = boundary(cell.atom)?.start();
        let name = symbol.symbol();
        if llvm.get_global(name.as_str()).is_some() || llvm.get_function(name.as_str()).is_some() {
            return Err(CodegenError(format!(
                "Context slot cell `{name}` is already declared"
            )));
        }
        let global = llvm.add_global(i64, None, name.as_str());
        global.set_initializer(&i64.const_zero());
        global.set_alignment(8);
        global.set_section(Some(profile.writable_storage_section()));
        crate::emission::apply_persistent_linkage(&global, symbol, true)?;
        atoms.push(GlobalAtomMaterializationV1::new(cell.atom, global));
        entries.push(
            entry.const_named_struct(&[
                super::super::registration_identity::digest_value(
                    context,
                    digest,
                    cell.key.0.as_array(),
                )
                .into(),
                global.as_pointer_value().into(),
            ]),
        );
    }
    let atom = scoop_lir::context_key_table_atom(registration.body_definition_plan())
        .map_err(|error| CodegenError(error.to_string()))?;
    let symbol = boundary(atom)?.start();
    let name = symbol.symbol();
    if llvm.get_global(name.as_str()).is_some() || llvm.get_function(name.as_str()).is_some() {
        return Err(CodegenError(format!(
            "Context key table `{name}` is already declared"
        )));
    }
    let value = entry.const_array(&entries);
    let table = llvm.add_global(value.get_type(), None, name.as_str());
    table.set_initializer(&value);
    table.set_constant(true);
    table.set_alignment(8);
    crate::emission::apply_persistent_linkage(&table, symbol, true)?;
    atoms.push(GlobalAtomMaterializationV1::new(atom, table));
    Ok(table.as_pointer_value())
}
