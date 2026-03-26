//! IR Schema - categorized collection of referenced types.

use apollo_codegen_frontend::types::*;
use indexmap::IndexMap;
use std::collections::HashMap;

/// Schema representation with categorized type collections.
#[derive(Debug)]
pub struct Schema {
    pub referenced_types: ReferencedTypes,
    pub documentation: Option<String>,
}

/// Categorized collections of referenced types.
#[derive(Debug)]
pub struct ReferencedTypes {
    pub objects: Vec<GraphQLObjectType>,
    pub interfaces: Vec<GraphQLInterfaceType>,
    pub unions: Vec<GraphQLUnionType>,
    pub scalars: Vec<GraphQLScalarType>,
    pub custom_scalars: Vec<GraphQLScalarType>,
    pub enums: Vec<GraphQLEnumType>,
    pub input_objects: Vec<GraphQLInputObjectType>,
}

/// Built-in scalar type names.
const BUILTIN_SCALARS: &[&str] = &["String", "Int", "Float", "Boolean", "ID"];

impl Schema {
    /// Build schema from a CompilationResult's referenced types.
    pub fn from_referenced_types(
        types: &[GraphQLNamedType],
        documentation: Option<String>,
    ) -> Self {
        let mut objects = Vec::new();
        let mut interfaces = Vec::new();
        let mut unions = Vec::new();
        let mut scalars = Vec::new();
        let mut custom_scalars = Vec::new();
        let mut enums = Vec::new();
        let mut input_objects = Vec::new();

        for named_type in types {
            match named_type {
                GraphQLNamedType::Object(o) => objects.push(o.clone()),
                GraphQLNamedType::Interface(i) => interfaces.push(i.clone()),
                GraphQLNamedType::Union(u) => unions.push(u.clone()),
                GraphQLNamedType::Scalar(s) => {
                    if BUILTIN_SCALARS.contains(&s.name.as_str()) {
                        scalars.push(s.clone());
                    } else {
                        custom_scalars.push(s.clone());
                    }
                }
                GraphQLNamedType::Enum(e) => enums.push(e.clone()),
                GraphQLNamedType::InputObject(io) => input_objects.push(io.clone()),
            }
        }

        Schema {
            referenced_types: ReferencedTypes {
                objects,
                interfaces,
                unions,
                scalars,
                custom_scalars,
                enums,
                input_objects,
            },
            documentation,
        }
    }
}
