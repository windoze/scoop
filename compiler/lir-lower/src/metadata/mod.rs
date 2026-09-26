use super::*;

mod descriptors;
mod layouts;
mod representation;
mod scans;
mod types;

pub(super) use descriptors::*;
pub(super) use layouts::*;
pub(super) use representation::*;
pub(super) use scans::*;
pub(super) use types::*;

pub(super) fn exact_type_record<'module>(
    module: &'module mir::Module,
    ty: &mir::Type,
) -> &'module mir::SourceExactTypeRecord {
    if let Some(identity) = module.meta.source_exact_types.get(ty) {
        return identity.identity_record();
    }
    let location = match ty {
        mir::Type::Class(id) => mir::GeneratedExactTypeLocation::Class(*id),
        mir::Type::Enum(id, _) => mir::GeneratedExactTypeLocation::Enum(*id),
        _ => panic!("validated MIR is missing the source exact identity for {ty:?}"),
    };
    generated_exact_type_record(module, location)
}

pub(super) fn generated_exact_type_record(
    module: &mir::Module,
    location: mir::GeneratedExactTypeLocation,
) -> &mir::GeneratedExactTypeRecord {
    module
        .meta
        .generated_exact_types
        .get(location)
        .unwrap_or_else(|| panic!("validated MIR is missing exact identity for {location:?}"))
        .exact_record()
}
