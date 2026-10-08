use super::*;

pub(super) fn render_outbound_function(
    module: &Module,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
    function: &scoop_lir::ExternFunction,
    bridge: &scoop_lir::GeneratedBridgeEntryIdentity,
    signature: &scoop_lir::CFunctionType,
    result: scoop_lir::CResultAdaptation,
) -> Result<String, CodegenError> {
    let surface = CBridgeTypeSurface::for_function(module, signature)?;
    let renderer = CTypeRenderer::new(surface.function_types());
    let mut out = unit_prelude(module, plan, &surface)?;
    let capture_errno = result == scoop_lir::CResultAdaptation::CaptureErrno;
    if capture_errno {
        out.push_str("#include <errno.h>\n");
    }
    let parameter_types = signature
        .params
        .iter()
        .map(|parameter| renderer.declaration(parameter, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let prototype_parameters = if parameter_types.is_empty() {
        "void".to_string()
    } else {
        parameter_types.join(", ")
    };
    let declarator = format!("{}({prototype_parameters})", function.native_symbol);
    out.push_str("extern ");
    out.push_str(&renderer.return_declaration(&signature.return_type, &declarator)?);
    out.push_str(";\n");

    let has_result = !signature.return_type.is_void();
    let mut wrapper_parameters = Vec::new();
    if has_result {
        wrapper_parameters.push("void *result".to_string());
    }
    wrapper_parameters.extend(
        signature
            .params
            .iter()
            .enumerate()
            .map(|(index, _)| format!("const void *arg{index}")),
    );
    if wrapper_parameters.is_empty() {
        wrapper_parameters.push("void".to_string());
    }
    out.push_str(&format!(
        "{} {}({}) {{\n",
        if capture_errno { "int32_t" } else { "void" },
        bridge.symbol(),
        wrapper_parameters.join(", ")
    ));
    for (index, parameter) in signature.params.iter().enumerate() {
        let declaration = renderer.declaration(parameter, &format!("value{index}"))?;
        out.push_str(&format!(
            "  {declaration};\n  __builtin_memcpy(&value{index}, arg{index}, sizeof(value{index}));\n"
        ));
    }
    let arguments = (0..signature.params.len())
        .map(|index| format!("value{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    if capture_errno {
        out.push_str("  errno = 0;\n");
    }
    if has_result {
        let result_declaration =
            renderer.return_declaration(&signature.return_type, "native_result")?;
        out.push_str(&format!(
            "  {result_declaration} = {}({arguments});\n",
            function.native_symbol
        ));
    } else {
        out.push_str(&format!("  {}({arguments});\n", function.native_symbol));
    }
    if capture_errno {
        out.push_str("  int captured_errno = errno;\n");
    }
    if has_result {
        out.push_str("  __builtin_memcpy(result, &native_result, sizeof(native_result));\n");
    }
    if capture_errno {
        out.push_str("  return (int32_t)captured_errno;\n");
    }
    out.push_str("}\n");
    Ok(out)
}
