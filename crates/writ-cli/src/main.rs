//! writ — Authorization and provenance for AI agents.
//! Nothing runs without a writ.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod cmds;

#[derive(Parser)]
#[command(
    name = "writ",
    version,
    about = "Authorization and provenance for AI agents. One policy file, one tamper-evident ledger, any agent.",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Policy file (default: ./writ.yaml).
    #[arg(long, global = true, default_value = "writ.yaml")]
    policy: PathBuf,

    /// Ledger path (default: ./.writ/ledger.jsonl).
    #[arg(long, global = true, default_value = ".writ/ledger.jsonl")]
    ledger: PathBuf,

    /// Flip the policy default from ask to allow. Dangerous; for demos only.
    #[arg(long, global = true)]
    yolo: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Wrap an agent, policy enforced: `writ run -- claude`
    Run {
        /// Sandbox backend (writ doctor lists what this machine supports).
        #[arg(long, default_value = "local-os")]
        backend: String,
        /// The agent command line, after `--`.
        #[arg(last = true, required = true)]
        cmd: Vec<String>,
    },

    /// Sit in front of an MCP server: `writ proxy --mcp --server github -- npx server`
    Proxy {
        #[arg(long)]
        mcp: bool,
        /// Downstream server identity (used for policy + credentials).
        #[arg(long, required = true)]
        server: String,
        /// Downstream server command line, after `--`.
        #[arg(last = true, required = true)]
        cmd: Vec<String>,
    },

    /// What did my agent actually do?
    Log,

    /// One decision, in full.
    Show { call_id: String },

    /// Verify the local ledger hash chain and name the first broken record.
    Verify,

    /// Policy operations.
    Policy {
        #[command(subcommand)]
        sub: PolicyCmd,
    },

    /// Reproduce a trajectory / replay a candidate policy against it.
    Replay {
        /// Session id (see `writ log`).
        session: String,
        /// Candidate policy to evaluate against the recorded calls.
        #[arg(long)]
        candidate: Option<PathBuf>,
        /// Re-branch from this decision index; irreversible steps refuse
        /// unless --ack-irreversible is passed.
        #[arg(long)]
        branch_from: Option<u64>,
        /// Explicit acknowledgement for irreversible steps (spec §10).
        #[arg(long)]
        ack_irreversible: bool,
    },

    /// Coverage report: which call paths are governed and which are blind.
    Doctor,

    /// Shareable single-file HTML run summary.
    Report {
        #[arg(long, default_value = "writ-report.html")]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum PolicyCmd {
    /// Unit-test your rules against recorded fixtures.
    Test {
        /// Fixture directory (contains *.yaml test cases).
        #[arg(long)]
        fixtures: Option<PathBuf>,
    },
    /// Install a community policy pack from packs/.
    Add { pack: String },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Run { backend, cmd } => {
            cmds::run(&cli.policy, &cli.ledger, cli.yolo, &backend, &cmd)
        }
        Commands::Proxy { mcp, server, cmd } => {
            cmds::proxy(&cli.policy, &cli.ledger, cli.yolo, mcp, &server, &cmd)
        }
        Commands::Log => cmds::log(&cli.ledger),
        Commands::Show { call_id } => cmds::show(&cli.ledger, &call_id),
        Commands::Verify => cmds::verify(&cli.ledger),
        Commands::Policy { sub } => match sub {
            PolicyCmd::Test { fixtures } => cmds::policy_test(&cli.policy, fixtures),
            PolicyCmd::Add { pack } => cmds::policy_add(&pack),
        },
        Commands::Doctor => cmds::doctor(&cli.policy, &cli.ledger),
        Commands::Replay {
            session,
            candidate,
            branch_from,
            ack_irreversible,
        } => cmds::replay(
            &cli.ledger,
            &session,
            candidate,
            branch_from,
            ack_irreversible,
        ),
        Commands::Report { out } => cmds::report(&cli.ledger, &out),
    }
}
