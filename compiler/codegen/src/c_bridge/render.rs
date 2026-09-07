use super::*;

pub(super) fn integer_name(kind: scoop_lir::IntegerKind) -> &'static str {
    match (kind.signedness(), kind.width()) {
        (scoop_lir::IntegerSignedness::Signed, scoop_lir::IntegerWidth::W8) => "int8_t",
        (scoop_lir::IntegerSignedness::Signed, scoop_lir::IntegerWidth::W16) => "int16_t",
        (scoop_lir::IntegerSignedness::Signed, scoop_lir::IntegerWidth::W32) => "int32_t",
        (scoop_lir::IntegerSignedness::Signed, scoop_lir::IntegerWidth::W64) => "int64_t",
        (scoop_lir::IntegerSignedness::Unsigned, scoop_lir::IntegerWidth::W8) => "uint8_t",
        (scoop_lir::IntegerSignedness::Unsigned, scoop_lir::IntegerWidth::W16) => "uint16_t",
        (scoop_lir::IntegerSignedness::Unsigned, scoop_lir::IntegerWidth::W32) => "uint32_t",
        (scoop_lir::IntegerSignedness::Unsigned, scoop_lir::IntegerWidth::W64) => "uint64_t",
    }
}

fn exact_data_pointee<'a>(
    pointee: &'a scoop_lir::CDataPointee,
    storage: &'a scoop_lir::CDataPointerStorage,
) -> Result<&'a scoop_lir::CDataPointee, CodegenError> {
    match storage {
        scoop_lir::CDataPointerStorage::Direct => Ok(pointee),
        scoop_lir::CDataPointerStorage::Nullable(reference) => (pointee == reference.pointee())
            .then_some(reference.pointee())
            .ok_or_else(|| {
                CodegenError(
                    "nullable C data-pointer renderer received mismatched exact pointee"
                        .to_string(),
                )
            }),
    }
}

fn exact_code_signature<'a>(
    signature: &'a scoop_lir::CFunctionType,
    storage: &'a scoop_lir::CCodePointerStorage,
) -> Result<&'a scoop_lir::CFunctionType, CodegenError> {
    match storage {
        scoop_lir::CCodePointerStorage::Direct => Ok(signature),
        scoop_lir::CCodePointerStorage::Nullable(reference) => (signature == reference.signature())
            .then_some(reference.signature())
            .ok_or_else(|| {
                CodegenError(
                    "nullable C code-pointer renderer received mismatched exact signature"
                        .to_string(),
                )
            }),
    }
}

fn collect_return_function_types(
    ty: &scoop_lir::CReturnType,
    found: &mut Vec<scoop_lir::CFunctionType>,
) -> Result<(), CodegenError> {
    if let scoop_lir::CReturnType::Value(ty) = ty {
        collect_function_types_from_type(ty, found)?;
    }
    Ok(())
}

fn collect_function_types_from_type(
    ty: &scoop_lir::CType,
    found: &mut Vec<scoop_lir::CFunctionType>,
) -> Result<(), CodegenError> {
    match ty {
        scoop_lir::CType::DataPointer { pointee, storage } => {
            let pointee = exact_data_pointee(pointee, storage)?;
            if let scoop_lir::CDataPointee::Object(pointee) = pointee {
                collect_function_types_from_type(pointee, found)?;
            }
        }
        scoop_lir::CType::CodePointer { signature, storage } => {
            let signature = exact_code_signature(signature, storage)?;
            for parameter in &signature.params {
                collect_function_types_from_type(parameter, found)?;
            }
            collect_return_function_types(&signature.return_type, found)?;
            if !found.contains(signature) {
                found.push(signature.clone());
            }
        }
        scoop_lir::CType::Integer(_) | scoop_lir::CType::Boolean | scoop_lir::CType::Struct(_) => {}
    }
    Ok(())
}

pub(super) fn collect_module_function_types(
    module: &Module,
) -> Result<Vec<scoop_lir::CFunctionType>, CodegenError> {
    let mut found = Vec::new();
    for (_, definition) in module.structs.iter() {
        if let Some(fields) = definition.c_fields() {
            for field in fields {
                collect_function_types_from_type(&field.ty, &mut found)?;
            }
        }
    }
    for (_, function) in module.extern_functions.iter() {
        if let ExternFunctionKind::C { signature, .. } = &function.kind {
            for parameter in &signature.params {
                collect_function_types_from_type(parameter, &mut found)?;
            }
            collect_return_function_types(&signature.return_type, &mut found)?;
        }
    }
    for (_, global) in module.native_globals.iter() {
        collect_function_types_from_type(&global.c_type, &mut found)?;
    }
    for (_, callback) in module.callback_bridges.iter() {
        for parameter in &callback.params {
            collect_function_types_from_type(parameter, &mut found)?;
        }
        collect_return_function_types(&callback.return_type, &mut found)?;
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        for parameter in &callback.params {
            collect_function_types_from_type(parameter, &mut found)?;
        }
        collect_return_function_types(&callback.return_type, &mut found)?;
    }
    Ok(found)
}

pub(super) struct CTypeRenderer<'a> {
    function_types: &'a [scoop_lir::CFunctionType],
}

impl<'a> CTypeRenderer<'a> {
    pub(super) const fn new(function_types: &'a [scoop_lir::CFunctionType]) -> Self {
        Self { function_types }
    }

    fn attach(base: &str, declarator: &str) -> String {
        if declarator.is_empty() {
            base.to_string()
        } else {
            format!("{base} {declarator}")
        }
    }

    pub(super) fn declaration(
        &self,
        ty: &scoop_lir::CType,
        declarator: &str,
    ) -> Result<String, CodegenError> {
        match ty {
            scoop_lir::CType::Integer(kind) => Ok(Self::attach(integer_name(*kind), declarator)),
            scoop_lir::CType::Boolean => Ok(Self::attach("_Bool", declarator)),
            scoop_lir::CType::DataPointer { pointee, storage } => {
                let pointee = exact_data_pointee(pointee, storage)?;
                let pointer = format!("*{declarator}");
                match pointee {
                    scoop_lir::CDataPointee::OpaqueVoid => Ok(Self::attach("void", &pointer)),
                    scoop_lir::CDataPointee::Object(pointee) => self.declaration(pointee, &pointer),
                }
            }
            scoop_lir::CType::CodePointer { signature, storage } => {
                let signature = exact_code_signature(signature, storage)?;
                let index = self
                    .function_types
                    .iter()
                    .position(|candidate| candidate == signature)
                    .ok_or_else(|| {
                        CodegenError(
                            "C function-pointer signature was not collected before rendering"
                                .to_string(),
                        )
                    })?;
                Ok(Self::attach(&format!("scoop_c_funptr_{index}"), declarator))
            }
            scoop_lir::CType::Struct(reference) => Ok(Self::attach(
                &format!("scoop_c_layout_{}", arena_index(reference.definition())),
                declarator,
            )),
        }
    }

    pub(super) fn return_declaration(
        &self,
        ty: &scoop_lir::CReturnType,
        declarator: &str,
    ) -> Result<String, CodegenError> {
        match ty {
            scoop_lir::CReturnType::Void => Ok(Self::attach("void", declarator)),
            scoop_lir::CReturnType::Value(ty) => self.declaration(ty, declarator),
        }
    }

    pub(super) fn function_pointer_typedefs(&self) -> Result<String, CodegenError> {
        let mut out = String::new();
        for (index, signature) in self.function_types.iter().enumerate() {
            let params = if signature.params.is_empty() {
                "void".to_string()
            } else {
                signature
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| self.declaration(parameter, &format!("arg{index}")))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            };
            let declarator = format!("(*scoop_c_funptr_{index})({params})");
            out.push_str("typedef ");
            out.push_str(&self.return_declaration(&signature.return_type, &declarator)?);
            out.push_str(";\n");
        }
        if !self.function_types.is_empty() {
            out.push('\n');
        }
        Ok(out)
    }
}

pub(super) fn c_struct_forward_declarations(module: &Module) -> String {
    let mut out = String::new();
    for (id, definition) in module.structs.iter() {
        if definition.is_c_layout() {
            let name = format!("scoop_c_layout_{}", arena_index(id));
            out.push_str(&format!("typedef struct {name} {name};\n"));
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}
