use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

use pint::commands::{self, transactions::Filters};

#[derive(Parser)]
#[command(name = "pint")]
#[command(about = "Personal finance transaction manager using SimpleFIN")]
#[command(version)]
struct Cli {
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

    match cli.command {
        Commands::Init => commands::init::run(),

        Commands::Setup => commands::setup::run(),

        Commands::Sync { days } => {
            let days = days.min(60);
            commands::sync::run(days)
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
