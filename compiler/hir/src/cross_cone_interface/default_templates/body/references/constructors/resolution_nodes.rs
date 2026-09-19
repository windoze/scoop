use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultClassConstructorIdV1, node, children, {
    match node {
        Self::Source(field_0) => {
            children.push(field_0)?;
        }
        Self::Generated(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultConstructorRefV1, node, children, {
    match node {
        Self::Struct {
            declaration,
            owner_type,
        } => {
            children.push(owner_type)?;
            children.push(declaration)?;
        }
        Self::Class {
            declaration,
            owner_type,
        } => {
            children.push(owner_type)?;
            children.push(declaration)?;
        }
        Self::Variant {
            declaration,
            owner_type,
        } => {
            children.push(owner_type)?;
            children.push(declaration)?;
        }
    }
    Ok(())
});
