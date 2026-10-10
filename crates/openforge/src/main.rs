//! `openforge`: the core process. The desktop app runs it as `openforge serve`;
//! scripts and AI assistants use its CLI and MCP subcommands.

mod mcp;
mod session;
mod views;

use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use of_protocol::{Response, parse_request};
use session::Session;

#[derive(Parser)]
#[command(name = "openforge", version, about = "OpenForge parametric CAD core")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve JSON-RPC 2.0 on stdin/stdout, one message per line (used by the desktop app).
    Serve,
    /// Run a Model Context Protocol server on stdin/stdout for AI assistants.
    Mcp {
        /// Part to open at start.
        part: Option<PathBuf>,
    },
    /// Rebuild a part and print its features, errors and mass properties.
    Info { part: PathBuf },
    /// Rebuild a part and export it to STEP, IGES or STL (chosen by extension).
    Export { part: PathBuf, output: PathBuf },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve => serve(),
        Command::Mcp { part } => {
            let session = match part {
                Some(p) => Session::open(p).map_err(anyhow::Error::msg)?,
                None => Session::new(),
            };
            mcp::run(session)
        }
        Command::Info { part } => info(part),
        Command::Export { part, output } => {
            let mut s = Session::open(part).map_err(anyhow::Error::msg)?;
            let r = s.handle("export", serde_json::json!({ "path": output }));
            match r {
                Ok(_) => {
                    println!("wrote {}", output.display());
                    Ok(())
                }
                Err(e) => bail!(e.message),
            }
        }
    }
}

fn serve() -> anyhow::Result<()> {
    let mut session = Session::new();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.context("reading stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = match parse_request(&line) {
            Ok(req) => {
                // A panic in one request must not take the app's core down.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    session.handle(&req.method, req.params)
                }));
                let Some(id) = req.id else { continue };
                match result {
                    Ok(Ok(v)) => Response::ok(id, v),
                    Ok(Err(e)) => Response::err(id, e),
                    Err(_) => Response::err(
                        id,
                        of_protocol::RpcError::failed(
                            "internal error; the operation was not completed",
                        ),
                    ),
                }
            }
            Err(resp) => resp,
        };
        writeln!(out, "{}", serde_json::to_string(&resp)?)?;
        out.flush()?;
    }
    Ok(())
}

fn info(part: PathBuf) -> anyhow::Result<()> {
    let mut s = Session::open(part).map_err(anyhow::Error::msg)?;
    let model = s.model();
    for (f, st) in model.doc.features.iter().zip(model.statuses()) {
        let state = if f.suppressed {
            "suppressed".to_string()
        } else if st.rolled_back {
            "rolled back".to_string()
        } else if let Some(e) = &st.error {
            format!("ERROR: {e}")
        } else if let Some(sk) = &st.sketch {
            sk.summary().to_string()
        } else {
            "ok".to_string()
        };
        println!("{:>4}  {:<24} {state}", f.id, f.name);
    }
    if let Ok(mp) = s.handle("mass_properties", serde_json::Value::Null) {
        println!();
        println!("Volume  {:.3} mm³", mp["volume"].as_f64().unwrap_or(0.0));
        println!("Area    {:.3} mm²", mp["area"].as_f64().unwrap_or(0.0));
        println!("Mass    {:.4} kg", mp["mass"].as_f64().unwrap_or(0.0));
    }
    Ok(())
}
