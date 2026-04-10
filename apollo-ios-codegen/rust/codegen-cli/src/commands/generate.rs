//! Generate command implementation.
//!
//! Mirrors Swift's `Sources/CodegenCLI/Commands/Generate.swift`.
//! Loads configuration, determines items to generate, and calls
//! `ApolloCodegen::build()` via the `CodegenProvider` trait.

use clap::Args;

use apollo_codegen_lib::codegen::{ApolloCodegen, CodegenProvider, ItemsToGenerate};
use apollo_codegen_lib::codegen_logger::CodegenLogger;

use crate::error::CliError;
use crate::input_options::{self, InputOptions};

/// Generate Swift source code based on a code generation configuration.
#[derive(Args, Debug)]
pub struct Generate {
    #[command(flatten)]
    pub inputs: InputOptions,

    /// Fetch the GraphQL schema before Swift code generation.
    #[arg(short, long)]
    pub fetch_schema: bool,
}

impl Generate {
    pub fn run(&self) -> Result<(), CliError> {
        CodegenLogger::set_level(self.inputs.verbose);

        // D-70: --fetch-schema is accepted but errors if used (stub)
        if self.fetch_schema {
            return Err(CliError::Generic {
                description: "Schema downloading is not yet supported in the Rust CLI. \
                              Use the Swift CLI for now."
                    .to_string(),
            });
        }

        let configuration = self.inputs.get_codegen_configuration()?;

        let mut items_to_generate = ItemsToGenerate::CODE;

        if let Some(ref manifest) = configuration.operation_manifest {
            if manifest.generate_manifest_on_code_generation {
                items_to_generate |= ItemsToGenerate::OPERATION_MANIFEST;
            }
        }

        let root_url = input_options::root_output_url(&self.inputs);

        ApolloCodegen::build(&configuration, root_url.as_deref(), items_to_generate)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Wrapper struct for testing Generate arg parsing.
    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        cmd: Generate,
    }

    #[test]
    fn test_generate_parses_fetch_schema_flag() {
        let cli = TestCli::try_parse_from(["test", "--fetch-schema"]).unwrap();
        assert!(cli.cmd.fetch_schema);
    }

    #[test]
    fn test_generate_default_no_fetch_schema() {
        let cli = TestCli::try_parse_from(["test"]).unwrap();
        assert!(!cli.cmd.fetch_schema);
    }

    #[test]
    fn test_generate_verbose_flag() {
        let cli = TestCli::try_parse_from(["test", "--verbose"]).unwrap();
        assert!(cli.cmd.inputs.verbose);
    }

    #[test]
    fn test_generate_fetch_schema_returns_stub_error() {
        let cmd = Generate {
            inputs: InputOptions {
                path: "./config.json".to_string(),
                string: None,
                verbose: false,
            },
            fetch_schema: true,
        };
        let result = cmd.run();
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Schema downloading is not yet supported"));
    }
}
