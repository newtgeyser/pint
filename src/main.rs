use std::path::PathBuf;

use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

use pint::commands::{self, transactions::Filters};
use pint::config;
use pint::tui;

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
    /// Launch interactive terminal UI
    Tui,

    /// Initialize database and configure SimpleFIN access
    Setup,

    /// Sync transactions from SimpleFIN
    Sync {
        /// Number of days to fetch (default: 30, max: 60)
        #[arg(short, long, default_value = "30")]
        days: u32,

        /// Backfill historical transactions (up to ~2 years)
        #[arg(long)]
        backfill: bool,

        /// Start backfill from this date (YYYY-MM-DD), requires --backfill
        #[arg(long, requires = "backfill")]
        from: Option<String>,
    },

    /// Manage accounts
    Accounts {
        #[command(subcommand)]
        action: Option<AccountsAction>,
    },

    /// List and filter transactions
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

    /// Set category on a single transaction
    Categorize {
        /// Transaction ID
        tx_id: String,
        /// Category name
        category: String,
    },

    /// Manage holdings (investment positions)
    Holdings {
        #[command(subcommand)]
        action: HoldingsAction,
    },

    /// Manage manual assets (real estate, vehicles, etc.)
    Assets {
        #[command(subcommand)]
        action: Option<AssetsAction>,
    },

    /// Manage categorization rules and categories
    Rules {
        #[command(subcommand)]
        action: Option<RulesAction>,
    },

    /// Manage reward points programs
    Points {
        #[command(subcommand)]
        action: Option<PointsAction>,
    },

    /// Show detected recurring transactions
    Recurring,

    /// Show net worth history
    History {
        /// Number of daily snapshots to show
        #[arg(short, long, default_value = "30")]
        limit: usize,
        /// Capture or refresh today's snapshot before listing
        #[arg(long)]
        capture: bool,
    },

    /// Show stale or incomplete financial data
    Health,

    /// Review and bulk-categorize uncategorized merchants
    Review {
        #[command(subcommand)]
        action: Option<ReviewAction>,
    },

    /// Manage reimbursing entities (employer, LLC, etc.)
    Reimbursers {
        #[command(subcommand)]
        action: Option<ReimbursersAction>,
    },

    /// Mark a transaction as reimbursable, paid, or clear it
    Reimburse {
        /// Transaction ID
        tx_id: String,
        /// Reimburser name (omit when using --paid or --clear)
        entity: Option<String>,
        /// Mark as paid back (reimbursed)
        #[arg(long, conflicts_with_all = ["entity", "clear"])]
        paid: bool,
        /// Clear the reimbursable marker
        #[arg(long, conflicts_with_all = ["entity", "paid"])]
        clear: bool,
    },

    /// List reimbursable transactions
    Reimbursable {
        /// Show only pending (not yet reimbursed)
        #[arg(long, conflicts_with = "paid")]
        pending: bool,
        /// Show only already reimbursed
        #[arg(long, conflicts_with = "pending")]
        paid: bool,
        /// Filter by reimburser name
        #[arg(short, long)]
        entity: Option<String>,
    },

    /// Reconcile reimbursement payments and outstanding balances
    Reimbursements {
        #[command(subcommand)]
        action: ReimbursementsAction,
    },
}

#[derive(Subcommand)]
enum ReimbursementsAction {
    /// Show outstanding reimbursements by age and entity
    Aging,
    /// Record a reimbursement payment, optionally allocating it to expenses
    Payment {
        entity: String,
        amount: f64,
        /// Payment date (YYYY-MM-DD)
        #[arg(long)]
        date: Option<String>,
        /// Imported credit transaction representing this payment
        #[arg(long)]
        source_transaction: Option<String>,
        #[arg(long)]
        reference: Option<String>,
        #[arg(long)]
        note: Option<String>,
        /// Allocate payment using TX_ID=AMOUNT; may be repeated
        #[arg(long = "allocate")]
        allocations: Vec<String>,
    },
}

#[derive(Subcommand)]
enum ReviewAction {
    /// List uncategorized merchant groups
    List {
        #[arg(short, long, default_value = "50")]
        limit: Option<usize>,
    },
    /// Categorize every uncategorized transaction in a merchant group
    Categorize {
        merchant: String,
        category: String,
        /// Create a merchant rule for future transactions
        #[arg(long)]
        rule: bool,
        /// Override the rule pattern (defaults to merchant)
        #[arg(long, requires = "rule")]
        pattern: Option<String>,
    },
}

#[derive(Subcommand)]
enum AccountsAction {
    /// Add a manual account
    Add {
        /// Account name
        name: String,
        /// Account type (checking, savings, credit, brokerage, retirement, loan, money market, unknown)
        #[arg(short = 't', long = "type")]
        account_type: String,
    },
    /// Remove an account and its transactions/holdings
    Remove {
        /// Account ID, nickname, or name (partial match)
        account: String,
    },
    /// Set an account's type
    SetType {
        /// Account ID, nickname, or name (partial match)
        account: String,
        /// Account type (checking, savings, credit, brokerage, retirement, loan, money market, unknown)
        account_type: String,
    },
    /// Set or clear an account's nickname
    Rename {
        /// Account ID, nickname, or name (partial match)
        account: String,
        /// Nickname to set (omit to clear)
        nickname: Option<String>,
    },
}

#[derive(Subcommand)]
enum HoldingsAction {
    /// List holdings
    List {
        /// Filter by account name or ID
        #[arg(short, long)]
        account: Option<String>,

        /// Sort by column: symbol, description, shares, price, value, gain, loss
        #[arg(short, long, default_value = "value")]
        sort: String,
    },
    /// Add a holding to an account
    Add {
        /// Account ID or name
        #[arg(short, long)]
        account: String,
        /// Symbol (e.g., BTC, AAPL)
        #[arg(short = 'S', long)]
        symbol: String,
        /// Number of shares/units
        #[arg(long)]
        shares: String,
        /// Price per share/unit in dollars
        #[arg(short, long)]
        price: f64,
        /// Total cost basis in dollars
        #[arg(short, long)]
        cost: Option<f64>,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Update a holding
    Update {
        /// Holding symbol or ID
        holding: String,
        /// New number of shares
        #[arg(long)]
        shares: Option<String>,
        /// New price per share
        #[arg(short, long)]
        price: Option<f64>,
        /// New cost basis
        #[arg(short, long)]
        cost: Option<f64>,
    },
    /// Remove a holding
    Remove {
        /// Holding symbol or ID
        holding: String,
    },
    /// Import holdings from a CSV file
    Import {
        /// Path to the CSV file
        file: PathBuf,
        /// Account ID or name to import into
        #[arg(short, long)]
        account: String,
    },
    /// Update prices for holdings in manual accounts from Yahoo Finance
    UpdatePrices,
}

#[derive(Subcommand)]
enum AssetsAction {
    /// Add a new asset
    Add {
        /// Asset name (e.g., "Primary Residence", "2020 Toyota Camry")
        name: String,
        /// Asset type: real_estate, vehicle, collectible, other
        #[arg(short = 't', long = "type")]
        asset_type: String,
        /// Current value in dollars
        #[arg(short, long)]
        value: f64,
        /// Cost basis in dollars (what you paid)
        #[arg(short, long)]
        cost: Option<f64>,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
        /// Acquisition date (YYYY-MM-DD)
        #[arg(short, long)]
        acquired: Option<String>,
    },
    /// Update an existing asset
    Update {
        /// Asset ID
        id: i64,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New value in dollars
        #[arg(short, long)]
        value: Option<f64>,
        /// New cost basis in dollars
        #[arg(short, long)]
        cost: Option<f64>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New asset type
        #[arg(short = 't', long = "type")]
        asset_type: Option<String>,
    },
    /// Remove an asset
    Remove {
        /// Asset ID
        id: i64,
    },
}

#[derive(Subcommand)]
enum RulesAction {
    /// List categories
    Categories,
    /// Apply rules to uncategorized transactions
    Apply,
    /// Categorize a transaction and create a rule for future matches
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

#[derive(Subcommand)]
enum ReimbursersAction {
    /// Create a new reimburser
    Add {
        /// Reimburser name (e.g., "Employer", "My LLC")
        name: String,
    },
    /// Remove a reimburser (must have no transactions referencing it)
    Remove {
        /// Reimburser name
        name: String,
    },
}

#[derive(Subcommand)]
enum PointsAction {
    /// Set points for a program (creates or updates)
    Set {
        /// Program name (e.g., "Chase Sapphire Reserve")
        program: String,
        /// Number of points
        points: i64,
        /// Optional note (e.g., "Ultimate Rewards")
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Add a new program
    Add {
        /// Program name
        program: String,
        /// Initial points balance
        #[arg(default_value = "0")]
        points: i64,
        /// Optional note
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Remove a program
    Remove {
        /// Program name
        program: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set custom data directory if provided
    if let Some(data_dir) = cli.data_dir {
        config::set_data_dir(data_dir);
    }

    match cli.command {
        Commands::Tui => tui::run(),

        Commands::Setup => commands::setup::run(),

        Commands::Sync {
            days,
            backfill,
            from,
        } => {
            if backfill {
                let from = from
                    .as_ref()
                    .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                    .transpose()?;
                commands::sync::run_backfill(from)?;
            } else {
                let days = if days > 60 {
                    eprintln!("Warning: days capped at 60 (SimpleFIN limit)");
                    60
                } else {
                    days
                };
                commands::sync::run(days)?;
            }
            // Auto-apply categorization rules after sync
            commands::categorize::run_auto()
        }

        Commands::Accounts { action } => match action {
            None => commands::accounts::run(),
            Some(AccountsAction::Add { name, account_type }) => {
                commands::accounts::add(&name, &account_type)
            }
            Some(AccountsAction::Remove { account }) => commands::accounts::remove(&account),
            Some(AccountsAction::SetType {
                account,
                account_type,
            }) => commands::accounts::set_type(&account, &account_type),
            Some(AccountsAction::Rename { account, nickname }) => {
                commands::accounts::set_nickname(&account, nickname.as_deref())
            }
        },

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

        Commands::Categorize { tx_id, category } => {
            commands::categorize::run_manual(&tx_id, &category)
        }

        Commands::Holdings { action } => match action {
            HoldingsAction::List { account, sort } => {
                commands::holdings::run(account.as_deref(), &sort)
            }
            HoldingsAction::Add {
                account,
                symbol,
                shares,
                price,
                cost,
                description,
            } => commands::holdings::add(
                &account,
                &symbol,
                &shares,
                price,
                cost,
                description.as_deref(),
            ),
            HoldingsAction::Update {
                holding,
                shares,
                price,
                cost,
            } => commands::holdings::update(&holding, shares.as_deref(), price, cost),
            HoldingsAction::Remove { holding } => commands::holdings::remove(&holding),
            HoldingsAction::Import { file, account } => {
                commands::import_holdings::run(&file, &account)
            }
            HoldingsAction::UpdatePrices => commands::prices::run(),
        },

        Commands::Assets { action } => match action {
            None => commands::assets::run(),
            Some(AssetsAction::Add {
                name,
                asset_type,
                value,
                cost,
                description,
                acquired,
            }) => {
                let acquired_ts = acquired
                    .as_ref()
                    .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                    .transpose()?
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
                commands::assets::add(
                    &name,
                    &asset_type,
                    value,
                    cost,
                    description.as_deref(),
                    acquired_ts,
                )
            }
            Some(AssetsAction::Update {
                id,
                name,
                value,
                cost,
                description,
                asset_type,
            }) => commands::assets::update(
                id,
                name.as_deref(),
                value,
                cost,
                description.as_deref(),
                asset_type.as_deref(),
            ),
            Some(AssetsAction::Remove { id }) => commands::assets::remove(id),
        },

        Commands::Rules { action } => match action {
            None => commands::rules::run(),
            Some(RulesAction::Categories) => commands::categories::run(),
            Some(RulesAction::Apply) => commands::categorize::run_auto(),
            Some(RulesAction::Learn {
                tx_id,
                category,
                pattern,
                r#match,
            }) => commands::categorize::run_learn(&tx_id, &category, &pattern, &r#match),
        },

        Commands::Points { action } => match action {
            None => commands::points::run(),
            Some(PointsAction::Set {
                program,
                points,
                note,
            }) => commands::points::set(&program, points, note.as_deref()),
            Some(PointsAction::Add {
                program,
                points,
                note,
            }) => commands::points::add(&program, points, note.as_deref()),
            Some(PointsAction::Remove { program }) => commands::points::remove(&program),
        },

        Commands::Recurring => commands::recurring::run(),

        Commands::History { limit, capture } => commands::insights::history(limit, capture),

        Commands::Health => commands::insights::health(),

        Commands::Review { action } => match action {
            None => commands::review::list(Some(50)),
            Some(ReviewAction::List { limit }) => commands::review::list(limit),
            Some(ReviewAction::Categorize {
                merchant,
                category,
                rule,
                pattern,
            }) => commands::review::categorize(&merchant, &category, rule, pattern.as_deref()),
        },

        Commands::Reimbursers { action } => match action {
            None => commands::reimbursers::run(),
            Some(ReimbursersAction::Add { name }) => commands::reimbursers::add(&name),
            Some(ReimbursersAction::Remove { name }) => commands::reimbursers::remove(&name),
        },

        Commands::Reimburse {
            tx_id,
            entity,
            paid,
            clear,
        } => {
            if clear {
                commands::reimburse::clear(&tx_id)
            } else if paid {
                commands::reimburse::mark_paid(&tx_id)
            } else if let Some(entity) = entity {
                commands::reimburse::set(&tx_id, &entity)
            } else {
                anyhow::bail!("Must provide an entity name, --paid, or --clear")
            }
        }

        Commands::Reimbursable {
            pending,
            paid,
            entity,
        } => {
            let filter = if pending {
                pint::db::models::ReimbursableFilter::Pending
            } else if paid {
                pint::db::models::ReimbursableFilter::Paid
            } else {
                pint::db::models::ReimbursableFilter::All
            };
            commands::reimburse::list(filter, entity.as_deref())
        }

        Commands::Reimbursements { action } => match action {
            ReimbursementsAction::Aging => commands::settlements::aging(),
            ReimbursementsAction::Payment {
                entity,
                amount,
                date,
                source_transaction,
                reference,
                note,
                allocations,
            } => commands::settlements::record_payment(
                &entity,
                amount,
                date.as_deref(),
                source_transaction.as_deref(),
                reference.as_deref(),
                note.as_deref(),
                &allocations,
            ),
        },
    }
}
