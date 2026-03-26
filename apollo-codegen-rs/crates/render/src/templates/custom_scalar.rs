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

use super::header;

pub fn render(
    type_name: &str,
    description: Option<&str>,
    specified_by_url: Option<&str>,
    access_modifier: &str,
    api_target_name: &str,
) -> String {
    let mut result = String::new();

    // Editable file header
    result.push_str(
        "// @generated\n\
         // This file was automatically generated and can be edited to\n\
         // implement advanced custom scalar functionality.\n\
         //\n\
         // Any changes to this file will not be overwritten by future\n\
         // code generation execution.\n",
    );
    result.push('\n');
    result.push_str(&format!("import {}\n", api_target_name));

    // Documentation comment
    let mut doc = description.map(|s| s.to_string());
    if let Some(url) = specified_by_url {
        let spec_docs = format!("Specified by: []({})", url);
        doc = Some(match doc {
            Some(d) => format!("{}\n\n{}", d, spec_docs),
            None => spec_docs,
        });
    }

    result.push('\n');
    if let Some(ref d) = doc {
        for line in d.lines() {
            if line.is_empty() {
                result.push_str("///\n");
            } else {
                result.push_str(&format!("/// {}\n", line));
            }
        }
    }

    result.push_str(&format!(
        "{}typealias {} = String\n",
        access_modifier,
        crate::naming::first_uppercased(type_name),
    ));

    result
}
