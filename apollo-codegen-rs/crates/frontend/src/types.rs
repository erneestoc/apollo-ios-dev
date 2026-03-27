//! GraphQL type system types.
//!
//! Mirrors the Swift GraphQL type hierarchy used in CompilationResult.

use indexmap::IndexMap;

/// A named GraphQL type.
#[derive(Debug, Clone)]
pub enum GraphQLNamedType {
    Scalar(GraphQLScalarType),
    Object(GraphQLObjectType),
    Interface(GraphQLInterfaceType),
    Union(GraphQLUnionType),
    Enum(GraphQLEnumType),
    InputObject(GraphQLInputObjectType),
}

impl GraphQLNamedType {
    pub fn name(&self) -> &str {
        match self {
            Self::Scalar(t) => &t.name,
            Self::Object(t) => &t.name,
            Self::Interface(t) => &t.name,
            Self::Union(t) => &t.name,
            Self::Enum(t) => &t.name,
            Self::InputObject(t) => &t.name,
        }
    }
}

/// A composite type (can have selection sets).
#[derive(Debug, Clone)]
pub enum GraphQLCompositeType {
    Object(GraphQLObjectType),
    Interface(GraphQLInterfaceType),
    Union(GraphQLUnionType),
}

impl GraphQLCompositeType {
    pub fn name(&self) -> &str {
        match self {
            Self::Object(t) => &t.name,
            Self::Interface(t) => &t.name,
            Self::Union(t) => &t.name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphQLScalarType {
    pub name: String,
    pub description: Option<String>,
    pub specified_by_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphQLObjectType {
    pub name: String,
    pub description: Option<String>,
    pub fields: IndexMap<String, GraphQLField>,
    pub interfaces: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphQLInterfaceType {
    pub name: String,
    pub description: Option<String>,
    pub fields: IndexMap<String, GraphQLField>,
    pub interfaces: Vec<String>,
    pub implementing_objects: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphQLUnionType {
    pub name: String,
    pub description: Option<String>,
    pub member_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphQLEnumType {
    pub name: String,
    pub description: Option<String>,
    pub values: Vec<GraphQLEnumValue>,
}

#[derive(Debug, Clone)]
pub struct GraphQLEnumValue {
    pub name: String,
    pub description: Option<String>,
    pub deprecation_reason: Option<String>,
    pub is_deprecated: bool,
}

#[derive(Debug, Clone)]
pub struct GraphQLInputObjectType {
    pub name: String,
    pub description: Option<String>,
    pub fields: IndexMap<String, GraphQLInputField>,
    pub is_one_of: bool,
}

#[derive(Debug, Clone)]
pub struct GraphQLField {
    pub name: String,
    pub field_type: GraphQLType,
    pub description: Option<String>,
    pub deprecation_reason: Option<String>,
    pub is_deprecated: bool,
    pub arguments: Vec<GraphQLArgument>,
}

#[derive(Debug, Clone)]
pub struct GraphQLInputField {
    pub name: String,
    pub field_type: GraphQLType,
    pub description: Option<String>,
    pub default_value: Option<GraphQLValue>,
    pub deprecation_reason: Option<String>,
    pub is_deprecated: bool,
}

#[derive(Debug, Clone)]
pub struct GraphQLArgument {
    pub name: String,
    pub argument_type: GraphQLType,
    pub default_value: Option<GraphQLValue>,
    pub deprecation_reason: Option<String>,
    pub is_deprecated: bool,
}

/// A variable definition on an operation.
#[derive(Debug, Clone)]
pub struct VariableDefinition {
    pub name: String,
    pub variable_type: GraphQLType,
    pub default_value: Option<GraphQLValue>,
}

/// A GraphQL type reference (wrapping named types with list/non-null modifiers).
#[derive(Debug, Clone)]
pub enum GraphQLType {
    Named(String),
    NonNull(Box<GraphQLType>),
    List(Box<GraphQLType>),
}

impl GraphQLType {
    pub fn named_type(&self) -> &str {
        match self {
            Self::Named(name) => name,
            Self::NonNull(inner) | Self::List(inner) => inner.named_type(),
        }
    }

    /// Returns true if this type contains a List wrapper at any level.
    pub fn is_list(&self) -> bool {
        match self {
            Self::List(_) => true,
            Self::NonNull(inner) => inner.is_list(),
            Self::Named(_) => false,
        }
    }
}

/// A GraphQL value (for default values, arguments, etc.).
#[derive(Debug, Clone)]
pub enum GraphQLValue {
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    Null,
    Enum(String),
    List(Vec<GraphQLValue>),
    Object(IndexMap<String, GraphQLValue>),
    Variable(String),
}

/// A compiled selection set.
#[derive(Debug)]
pub struct SelectionSet {
    pub parent_type: GraphQLCompositeType,
    pub selections: Vec<Selection>,
}

/// A selection within a selection set.
#[derive(Debug)]
pub enum Selection {
    Field(Field),
    InlineFragment(InlineFragment),
    FragmentSpread(FragmentSpread),
}

/// A field selection.
#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub field_type: GraphQLType,
    pub arguments: Option<Vec<Argument>>,
    pub directives: Option<Vec<Directive>>,
    pub inclusion_conditions: Option<Vec<InclusionCondition>>,
    pub selection_set: Option<SelectionSet>,
}

/// An inline fragment (type condition or directive-based).
#[derive(Debug)]
pub struct InlineFragment {
    pub type_condition: Option<GraphQLCompositeType>,
    pub selection_set: SelectionSet,
    pub directives: Option<Vec<Directive>>,
    pub inclusion_conditions: Option<Vec<InclusionCondition>>,
}

/// A named fragment spread.
#[derive(Debug)]
pub struct FragmentSpread {
    pub fragment_name: String,
    pub directives: Option<Vec<Directive>>,
    pub inclusion_conditions: Option<Vec<InclusionCondition>>,
}

/// An argument on a field or directive.
#[derive(Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub value: GraphQLValue,
}

/// A directive applied to a field, fragment, or operation.
#[derive(Debug, Clone)]
pub struct Directive {
    pub name: String,
    pub arguments: Option<Vec<Argument>>,
}

/// An inclusion condition (@skip / @include).
#[derive(Debug, Clone)]
pub struct InclusionCondition {
    pub variable: String,
    pub is_inverted: bool,
}
