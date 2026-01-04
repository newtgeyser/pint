use std::path::PathBuf;

use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

use pint::commands::{self, transactions::Filters};
use pint::config;

#[derive(Parser)]
#[command(name = "pint")]
#[command(about = "Personal finance transaction manager using SimpleFIN")]
#[command(version)]
struct Cli {
    /// Data directory (default: ~/.local/share/pint or $PINT_DATA_DIR)
    #[arg(long, global = true, env = "PINT_DATA_DIR")]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the database
    Init,

    /// Configure SimpleFIN access
    Setup,

    /// Sync transactions from SimpleFIN
    Sync {
        /// Number of days to fetch (default: 30, max: 60)
        #[arg(short, long, default_value = "30")]
        days: u32,
    },

    /// Backfill historical transactions (up to ~2 years)
    Backfill {
        /// Start from this date instead of today (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,
    },

    /// List accounts
    Accounts,

    /// List transactions
    Transactions {
        /// Filter by account name or ID
        #[arg(short, long)]
        account: Option<String>,

        /// Filter from date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// Filter to date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,

        /// Search in description
        #[arg(short, long)]
        search: Option<String>,

        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,

        /// Show only uncategorized transactions
        #[arg(short, long)]
        uncategorized: bool,

        /// Limit number of results
        #[arg(short, long, default_value = "50")]
        limit: Option<usize>,
    },

    /// List categories
    Categories,

    /// List merchant rules
    Rules,

    /// Import/reload merchant rules from config file
    ImportRules,

    /// Categorize transactions
    Categorize {
        #[command(subcommand)]
        action: CategorizeAction,
    },
}

#[derive(Subcommand)]
enum CategorizeAction {
    /// Auto-categorize uncategorized transactions using rules
    Auto,

    /// Manually set a transaction's category
    Set {
        /// Transaction ID
        tx_id: String,
        /// Category name
        category: String,
    },

    /// Set category and create a rule for future matches
    Learn {
        /// Transaction ID
        tx_id: String,
        /// Category name
        category: String,
        /// Pattern to match (case-insensitive)
        pattern: String,
        /// Match mode: substring (default) or token
        #[arg(long, default_value = "substring")]
        r#match: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set custom data directory if provided
    if let Some(data_dir) = cli.data_dir {
        config::set_data_dir(data_dir);
    }

    match cli.command {
        Commands::Init => commands::init::run(),

        Commands::Setup => commands::setup::run(),

        Commands::Sync { days } => {
            let days = days.min(60);
            commands::sync::run(days)
        }

        Commands::Backfill { from } => {
            let from = from
                .as_ref()
                .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                .transpose()?;
            commands::sync::run_backfill(from)
        }

        Commands::Accounts => commands::accounts::run(),

        Commands::Transactions {
            account,
            from,
            to,
            search,
            category,
            uncategorized,
            limit,
        } => {
            let from = from
                .as_ref()
                .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                .transpose()?;
            let to = to
                .as_ref()
                .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                .transpose()?;

            commands::transactions::run(Filters {
                account,
                from,
                to,
                search,
                category,
                uncategorized,
                limit,
            })
        }

        Commands::Categories => commands::categories::run(),

        Commands::Rules => commands::rules::run(),

        Commands::ImportRules => commands::import_rules::run(),

        Commands::Categorize { action } => match action {
            CategorizeAction::Auto => commands::categorize::run_auto(),
            CategorizeAction::Set { tx_id, category } => {
                commands::categorize::run_manual(&tx_id, &category)
            }
            CategorizeAction::Learn {
                tx_id,
                category,
                pattern,
                r#match,
            } => commands::categorize::run_learn(&tx_id, &category, &pattern, &r#match),
        },
    }
}
