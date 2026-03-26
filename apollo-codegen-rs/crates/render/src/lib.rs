//! Swift code rendering for Apollo iOS code generation.
//!
//! Contains the TemplateString engine, all templates, and file generators
//! that produce .graphql.swift files from IR.

pub mod ir_adapter;
pub mod naming;
pub mod template_string;
pub mod templates;
