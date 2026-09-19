use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultLocalFunctionV1, node, children, {
    let DecodedDefaultLocalFunctionV1 {
        declaration,
        definition_path,
        function_type,
        captures,
        owner_type_parameter_count,
    } = node;
    children.push(owner_type_parameter_count)?;
    children.push(captures)?;
    children.push(function_type)?;
    children.push(definition_path)?;
    children.push(declaration)?;
    Ok(())
});
