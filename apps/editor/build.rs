//! Compiles the Slint UI ahead of time (no interpreter at runtime).
//!
//! Debug builds and the `agent-inspect` feature also emit element debug info, which the
//! accessibility-tree UI tests and Slint's MCP server need to walk the element tree.

fn main() -> Result<(), slint_build::CompileError> {
    let debug_build = std::env::var("PROFILE").is_ok_and(|p| p == "debug");
    let agent_inspect = std::env::var_os("CARGO_FEATURE_AGENT_INSPECT").is_some();
    let config =
        slint_build::CompilerConfiguration::new().with_debug_info(debug_build || agent_inspect);
    slint_build::compile_with_config("ui/app.slint", config)
}
