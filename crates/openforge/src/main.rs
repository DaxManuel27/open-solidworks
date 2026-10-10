//! `openforge`: the core process. The desktop app runs it as `openforge serve`;
//! scripts and AI assistants use its CLI and MCP subcommands.

fn main() {
    println!("openforge {}", env!("CARGO_PKG_VERSION"));
}
