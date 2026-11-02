//! Compiles the Slint UI ahead of time (no interpreter at runtime), and on Windows embeds the
//! icon, version information and manifest into `tellshot-editor.exe`.
//!
//! Debug builds and the `agent-inspect` feature also emit element debug info, which the
//! accessibility-tree UI tests and Slint's MCP server need to walk the element tree.

fn main() -> Result<(), String> {
    let debug_build = std::env::var("PROFILE").is_ok_and(|p| p == "debug");
    let agent_inspect = std::env::var_os("CARGO_FEATURE_AGENT_INSPECT").is_some();
    let config =
        slint_build::CompilerConfiguration::new().with_debug_info(debug_build || agent_inspect);
    slint_build::compile_with_config("ui/app.slint", config).map_err(|e| e.to_string())?;
    tellshot_buildinfo::embed(tellshot_buildinfo::Resources {
        description: "Tellshot editor",
        file_name: "tellshot-editor.exe",
    })
}
