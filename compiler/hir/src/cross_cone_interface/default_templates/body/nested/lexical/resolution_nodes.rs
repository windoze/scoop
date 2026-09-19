use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultLexicalCallableV1, node, children, {
    let DecodedDefaultLexicalCallableV1 {
        body,
        definition_path,
        function_type,
        body_type_arguments,
        captures,
        owner_type_parameter_count,
    } = node;
    children.push(owner_type_parameter_count)?;
    children.push(captures)?;
    children.push(body_type_arguments)?;
    children.push(function_type)?;
    children.push(definition_path)?;
    children.push(body)?;
    Ok(())
});

resource_node!(DecodedDefaultLambdaV1, node, children, {
    let DecodedDefaultLambdaV1(field_0) = node;
    children.push(field_0)?;
    Ok(())
});

resource_node!(DecodedDefaultAnonymousFunctionV1, node, children, {
    let DecodedDefaultAnonymousFunctionV1(field_0) = node;
    children.push(field_0)?;
    Ok(())
});
