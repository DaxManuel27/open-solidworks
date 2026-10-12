//! `openforge`: the core process. The desktop app runs it as `openforge serve`;
//! scripts and AI assistants use its CLI and MCP subcommands.

mod asm;
mod bench;
mod mcp;
mod session;
mod tools;
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
    /// Rebuild a part (or solve an assembly) and print its features or
    /// components, errors and mass properties.
    Info { part: PathBuf },
    /// Rebuild a part or assembly and export it to STEP, IGES or STL (chosen by extension).
    Export {
        part: PathBuf,
        output: PathBuf,
        /// Export this configuration ID instead of the saved active variant.
        #[arg(long)]
        configuration: Option<u32>,
    },
    /// Regenerate the TypeScript tool types and protocol tables from the catalog.
    Generate {
        /// Repository root.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Only check that the committed files are current.
        #[arg(long)]
        check: bool,
    },
    /// Check a finished part against a benchmark task's geometry assertions.
    Check {
        /// Task file, such as bench/tasks/01-plate-holes.json.
        task: PathBuf,
        /// The part produced by the task.
        part: PathBuf,
    },
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
        Command::Generate { root, check } => {
            let stale = tools::generate::run(&root, check)?;
            match (check, stale.is_empty()) {
                (true, false) => bail!("stale generated files: {}", stale.join(", ")),
                (false, false) => println!("wrote {}", stale.join(", ")),
                _ => println!("generated files are current"),
            }
            Ok(())
        }
        Command::Check { task, part } => {
            if bench::run(&task, &part)? {
                Ok(())
            } else {
                bail!("task checks failed")
            }
        }
        Command::Export {
            part,
            output,
            configuration,
        } => {
            let mut s = Session::open(part).map_err(anyhow::Error::msg)?;
            let r = s.handle(
                "export",
                serde_json::json!({ "path": output, "configuration": configuration }),
            );
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
            Err(resp) => *resp,
        };
        writeln!(out, "{}", serde_json::to_string(&resp)?)?;
        out.flush()?;
    }
    Ok(())
}

fn info(part: PathBuf) -> anyhow::Result<()> {
    let mut s = Session::open(part).map_err(anyhow::Error::msg)?;
    if let Some(a) = s.assembly() {
        let doc = a.doc();
        let d = &a.asm.diagnostics;
        for c in &doc.components {
            let status = d
                .component_errors
                .get(&c.id.to_string())
                .map(|e| format!("ERROR: {e}"))
                .unwrap_or_else(|| match d.dof.get(&c.id.to_string()) {
                    _ if c.fixed => "fixed".into(),
                    Some(0) => "fully defined".into(),
                    Some(n) => format!("{n} DOF"),
                    None => "suppressed".into(),
                });
            println!("{:>4}  {:<24} {:<32} {status}", c.id, c.name, c.path);
        }
        for m in &doc.mates {
            let status = d
                .mate_errors
                .get(&m.id)
                .cloned()
                .unwrap_or_else(|| "ok".into());
            println!("{:>4}  {:<24} {status}", m.id, m.name);
        }
        if let Ok(mp) = s.handle("mass_properties", serde_json::Value::Null) {
            println!();
            println!("Mass    {:.4} kg", mp["mass"].as_f64().unwrap_or(0.0));
            println!("Volume  {:.3} mm³", mp["volume"].as_f64().unwrap_or(0.0));
        }
        return Ok(());
    }
    let model = s.model();
    for (f, st) in model.doc.features.iter().zip(model.statuses()) {
        let state = if st.suppressed {
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
