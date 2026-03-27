//! Custom scalar type template.
//!
//! Generates files like:
//! ```swift
//! // @generated
//! // This file was automatically generated and can be edited to
//! // implement advanced custom scalar functionality.
//! //
//! // Any changes to this file will not be overwritten by future
//! // code generation execution.
//!
//! import ApolloAPI
//!
//! public typealias CustomDate = String
//! ```

use askama::Template;

#[derive(Template)]
#[template(path = "custom_scalar.swift.askama", escape = "none")]
struct CustomScalarTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    type_name: String,
    documentation: Option<String>,
}

pub fn render(
    type_name: &str,
    description: Option<&str>,
    specified_by_url: Option<&str>,
    access_modifier: &str,
    api_target_name: &str,
) -> String {
    // Build documentation string
    let mut doc = description.map(|s| s.to_string());
    if let Some(url) = specified_by_url {
        let spec_docs = format!("Specified by: []({})", url);
        doc = Some(match doc {
            Some(d) => format!("{}\n\n{}", d, spec_docs),
            None => spec_docs,
        });
    }

    let template = CustomScalarTemplate {
        api_target_name,
        access_modifier,
        type_name: crate::naming::first_uppercased(type_name),
        documentation: doc,
    };

    let mut output = template.render().expect("custom_scalar template render failed");
    // Ensure trailing newline
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}
