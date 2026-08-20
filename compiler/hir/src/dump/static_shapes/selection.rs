use super::*;

pub(super) fn types(module: &Module) -> Vec<(TypeId, &[TypeParamDecl])> {
    let mut types = std::collections::BTreeMap::new();
    for (ty, _) in module.types.iter() {
        if module.type_identities[ty].exact().is_some() {
            types.insert(ty, &[][..]);
        }
    }
    for declaration in module.structs.values() {
        let ty = module.struct_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    for declaration in module.enums.values() {
        let ty = module.enum_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    for declaration in module.classes.values() {
        let ty = module.class_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    for declaration in module.interfaces.values() {
        let ty = module.interface_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    // Loaded declarations also carry open source shapes; they do not create
    // new machine roots merely because a dump can inspect them.
    for loaded in module.loaded_struct_definitions.values() {
        let declaration = &loaded.definition;
        let ty = module.struct_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    for loaded in module.loaded_enum_definitions.values() {
        let declaration = &loaded.definition;
        let ty = module.enum_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    for loaded in module.loaded_class_definitions.values() {
        let declaration = &loaded.definition;
        let ty = module.class_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    for loaded in module.loaded_interface_definitions.values() {
        let declaration = &loaded.definition;
        let ty = module.interface_applications[declaration.self_application].canonical_type;
        types.insert(ty, declaration.type_params.as_slice());
    }
    types.into_iter().collect()
}
