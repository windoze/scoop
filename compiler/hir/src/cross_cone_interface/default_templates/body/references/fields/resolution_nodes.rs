use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultEnumVariantRefV1, node, children, {
    let DecodedDefaultEnumVariantRefV1 {
        declaration,
        owner_type,
    } = node;
    children.push(owner_type)?;
    children.push(declaration)?;
    Ok(())
});

resource_node!(DecodedDefaultEnumVariantFieldRefV1, node, children, {
    let DecodedDefaultEnumVariantFieldRefV1 {
        declaration,
        owner_type,
    } = node;
    children.push(owner_type)?;
    children.push(declaration)?;
    Ok(())
});

resource_node!(DecodedDefaultFieldRefV1, node, children, {
    match node {
        Self::Struct {
            declaration,
            owner_type,
        } => {
            children.push(owner_type)?;
            children.push(declaration)?;
        }
        Self::Tuple { declaration_index } => {
            children.push(declaration_index)?;
        }
        Self::Class {
            declaration,
            owner_type,
        } => {
            children.push(owner_type)?;
            children.push(declaration)?;
        }
    }
    Ok(())
});
