//! Library half of the `truewright` binary -- exists so integration tests can
//! exercise pieces of the CLI (currently just `mcp`'s streamable-HTTP
//! transport) in-process, the same way `crates/engine`'s tests do, instead
//! of spawning the compiled binary as a subprocess.

pub mod mcp;
