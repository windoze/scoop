use super::*;

pub(super) fn dump_function(module: &Module, function: &Function, out: &mut String) {
    let function_type_params: Vec<_> = function.type_params().into_iter().cloned().collect();
    let type_params = if function_type_params.is_empty() {
        String::new()
    } else {
        dump_type_params(module, &function_type_params)
    };
    let params: Vec<String> = match function.kind {
        FunctionKind::Extern(id) => module.extern_functions[id]
            .params
            .iter()
            .enumerate()
            .map(|(index, &ty)| format!("arg{}: {}", index + 1, type_name(module, ty)))
            .collect(),
        _ => function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_name(module, p.ty)))
            .collect(),
    };
    let signature = format!(
        "{}{}({}): {}",
        function.name,
        type_params,
        params.join(", "),
        type_name(module, function.return_ty)
    );
    let suspend = if function.is_suspend { "suspend " } else { "" };
    let operator = if function.modifiers.operator.is_some()
        || function.modifiers.property_delegate_operator.is_some()
    {
        "operator "
    } else {
        ""
    };
    let mut attributes = dump_function_attributes(function.attributes);
    if let ReleaseCallability::NoTransition { requirements } = &function.release_callability {
        attributes.push_str(" <no-transition>");
        if !requirements.is_empty() {
            let parameters = requirements
                .iter()
                .map(|parameter| function.type_param(*parameter).name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            attributes.push_str(&format!(" <requires-release-value {parameters}>"));
        }
    }
    let no_gc_requirements = match &function.genericity {
        FunctionGenericity::Plain => &[][..],
        FunctionGenericity::Generic { definition, .. } => {
            &module.generic_functions[*definition].no_gc_type_params
        }
        FunctionGenericity::OwnerParameterizedMethod {
            no_gc_type_params, ..
        } => no_gc_type_params,
        FunctionGenericity::GenericMethod { definition, .. } => {
            &module.generic_methods[*definition].no_gc_type_params
        }
    };
    let no_gc_condition = if no_gc_requirements.is_empty() {
        String::new()
    } else {
        let parameters = no_gc_requirements
            .iter()
            .map(|parameter| function.type_param(*parameter).name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(" <requires-gc-free {parameters}>")
    };
    match &function.kind {
        FunctionKind::Intrinsic(intrinsic) => {
            out.push_str(&format!(
                "  {operator}{suspend}fun {signature}{attributes}{no_gc_condition} <intrinsic {}>\n",
                intrinsic.kind.name(),
            ));
        }
        FunctionKind::User(body) => {
            out.push_str(&format!(
                "  {operator}{suspend}fun {signature}{attributes}{no_gc_condition}\n"
            ));
            dump_statements(module, &body.locals, &body.statements, 2, out);
        }
        FunctionKind::Abstract { .. } | FunctionKind::InitializationEnsure => {
            out.push_str(&format!(
                "  {operator}{suspend}fun {signature}{attributes}{no_gc_condition}\n"
            ));
        }
        FunctionKind::DerivedEquality => {
            out.push_str(&format!(
                "  {operator}{suspend}fun {signature}{attributes}{no_gc_condition} <derived equality>\n"
            ));
        }
        FunctionKind::Extern(id) => {
            let extern_ = &module.extern_functions[*id];
            let abi = match extern_.abi {
                ExternAbi::C(CAbiCallMode::NativeSafe) => "c",
                ExternAbi::C(CAbiCallMode::GcLeaf) => "c gc-leaf",
                ExternAbi::Scoop => "scoop",
            };
            let library = if extern_.library.is_empty() {
                String::new()
            } else {
                format!(" lib={}", extern_.library)
            };
            out.push_str(&format!(
                "  fun {signature}{attributes}{no_gc_condition} <extern{} abi={abi} symbol={}{}>\n",
                id.into_raw(),
                extern_.native_symbol,
                library
            ));
        }
    }
}
