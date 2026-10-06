use super::*;

pub(super) struct CBridgeTypeSurface {
    function_types: Vec<scoop_lir::CFunctionType>,
    struct_declarations: HashSet<usize>,
    struct_definitions: HashSet<usize>,
}

impl CBridgeTypeSurface {
    pub(super) fn for_function(
        module: &Module,
        signature: &scoop_lir::CFunctionType,
    ) -> Result<Self, CodegenError> {
        let mut surface = Self::empty();
        for parameter in &signature.params {
            surface.visit_type(module, parameter, true)?;
        }
        surface.visit_return(module, &signature.return_type, true)?;
        Ok(surface)
    }

    pub(super) fn for_signature_parts(
        module: &Module,
        parameters: &[scoop_lir::CType],
        result: &scoop_lir::CReturnType,
    ) -> Result<Self, CodegenError> {
        let mut surface = Self::empty();
        surface.visit_types(module, parameters, result, true)?;
        Ok(surface)
    }

    pub(super) fn for_value(module: &Module, ty: &scoop_lir::CType) -> Result<Self, CodegenError> {
        let mut surface = Self::empty();
        surface.visit_type(module, ty, true)?;
        Ok(surface)
    }

    pub(super) fn for_module(module: &Module) -> Result<Self, CodegenError> {
        let mut surface = Self::empty();
        for (id, definition) in module.structs.iter() {
            if definition.is_c_layout() {
                surface.materialize_struct(module, id)?;
            }
        }
        for (_, function) in module.extern_functions.iter() {
            if let ExternFunctionKind::C { signature, .. } = &function.kind {
                surface.visit_function(module, signature, true)?;
            }
        }
        for (_, global) in module.native_globals.iter() {
            surface.visit_type(module, &global.c_type, true)?;
        }
        for (_, callback) in module.callback_bridges.iter() {
            surface.visit_types(module, &callback.params, &callback.return_type, true)?;
        }
        for (_, callback) in module.foreign_callback_bridges.iter() {
            surface.visit_types(module, &callback.params, &callback.return_type, true)?;
        }
        Ok(surface)
    }

    fn empty() -> Self {
        Self {
            function_types: Vec::new(),
            struct_declarations: HashSet::new(),
            struct_definitions: HashSet::new(),
        }
    }

    fn visit_types(
        &mut self,
        module: &Module,
        parameters: &[scoop_lir::CType],
        result: &scoop_lir::CReturnType,
        materialize_structs: bool,
    ) -> Result<(), CodegenError> {
        for parameter in parameters {
            self.visit_type(module, parameter, materialize_structs)?;
        }
        self.visit_return(module, result, materialize_structs)
    }

    fn visit_function(
        &mut self,
        module: &Module,
        signature: &scoop_lir::CFunctionType,
        materialize_structs: bool,
    ) -> Result<(), CodegenError> {
        self.visit_types(
            module,
            &signature.params,
            &signature.return_type,
            materialize_structs,
        )?;
        if !self.function_types.contains(signature) {
            self.function_types.push(signature.clone());
        }
        Ok(())
    }

    fn visit_return(
        &mut self,
        module: &Module,
        ty: &scoop_lir::CReturnType,
        materialize_structs: bool,
    ) -> Result<(), CodegenError> {
        if let scoop_lir::CReturnType::Value(ty) = ty {
            self.visit_type(module, ty, materialize_structs)?;
        }
        Ok(())
    }

    fn visit_type(
        &mut self,
        module: &Module,
        ty: &scoop_lir::CType,
        materialize_structs: bool,
    ) -> Result<(), CodegenError> {
        match ty {
            scoop_lir::CType::DataPointer { pointee, storage } => {
                let pointee = exact_data_pointee(pointee, storage)?;
                if let scoop_lir::CDataPointee::Object(pointee) = pointee {
                    self.visit_type(module, pointee, false)?;
                }
            }
            scoop_lir::CType::CodePointer { signature, storage } => {
                let signature = exact_code_signature(signature, storage)?;
                self.visit_function(module, signature, false)?;
            }
            scoop_lir::CType::Struct(reference) => {
                let id = reference.definition();
                self.struct_declarations.insert(arena_index(id));
                if materialize_structs {
                    self.materialize_struct(module, id)?;
                }
            }
            scoop_lir::CType::Float(_)
            | scoop_lir::CType::Integer(_)
            | scoop_lir::CType::Boolean => {}
        }
        Ok(())
    }

    fn materialize_struct(
        &mut self,
        module: &Module,
        id: scoop_lir::StructDefId,
    ) -> Result<(), CodegenError> {
        let raw = arena_index(id);
        self.struct_declarations.insert(raw);
        if !self.struct_definitions.insert(raw) {
            return Ok(());
        }
        let fields = module.structs[id].c_fields().ok_or_else(|| {
            CodegenError(format!(
                "ordinary struct `{}` entered the generated C type surface",
                module.structs[id].name
            ))
        })?;
        for field in fields {
            self.visit_type(module, &field.ty, true)?;
        }
        Ok(())
    }

    pub(super) fn function_types(&self) -> &[scoop_lir::CFunctionType] {
        &self.function_types
    }

    pub(super) fn declares_struct(&self, id: scoop_lir::StructDefId) -> bool {
        self.struct_declarations.contains(&arena_index(id))
    }

    pub(super) fn defines_struct(&self, id: scoop_lir::StructDefId) -> bool {
        self.struct_definitions.contains(&arena_index(id))
    }

    pub(super) fn definition_count(&self) -> usize {
        self.struct_definitions.len()
    }
}

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
            scoop_lir::CType::Float(kind) => Ok(Self::attach(
                match kind {
                    scoop_lir::FloatKind::F32 => "float",
                    scoop_lir::FloatKind::F64 => "double",
                },
                declarator,
            )),
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

pub(super) fn c_struct_forward_declarations(
    module: &Module,
    surface: &CBridgeTypeSurface,
) -> String {
    let mut out = String::new();
    for (id, definition) in module.structs.iter() {
        if definition.is_c_layout() && surface.declares_struct(id) {
            let name = format!("scoop_c_layout_{}", arena_index(id));
            out.push_str(&format!("typedef struct {name} {name};\n"));
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}
