# Pint

A personal finance transaction manager that imports data from [SimpleFIN](https://www.simplefin.org/) and stores it in a local SQLite database. Built to be slim, durable, and fully under your control—your data stays on your machine.

## Why Pint?

Mint-style financial services get bought, discontinue products and sell your data. Pint exists so you are not the product.
- Imports transactions from bank accounts without storing credentials
- Keeps all data locally (SQLite)
- Allows enrichment (categories, tags, transfer linking)

SimpleFIN acts as the bridge to financial institutions—you authenticate with them, and Pint just fetches the data.

## Installation

### From Source

Requires Rust 1.70+:

```bash
git clone https://github.com/yourusername/pint
cd pint
cargo install --path .
```

The binary is self-contained (~7MB) with SQLite bundled.

## Quick Start

```bash
# 1. Initialize the database
pint init

# 2. Import default categorization rules (377 common merchants)
pint import-rules

# 3. Get a setup token from https://beta-bridge.simplefin.org/
#    Connect your bank accounts there, then get a token
pint setup

# 4. Fetch transactions (last 30 days by default)
pint sync

# 5. Auto-categorize transactions using rules
pint categorize auto

# 6. View your data
pint accounts
pint transactions
```

## Commands

### Core

| Command | Description |
|---------|-------------|
| `pint init` | Initialize the database |
| `pint setup` | Configure SimpleFIN access (one-time) |
| `pint sync [--days N]` | Fetch transactions (default: 30, max: 60) |
| `pint backfill` | Fetch historical transactions (up to ~2 years) |

### Viewing Data

| Command | Description |
|---------|-------------|
| `pint accounts` | List accounts with balances |
| `pint transactions [options]` | List transactions with filters |
| `pint categories` | List all categories |
| `pint rules` | List merchant categorization rules |

#### Transaction Filters

```bash
pint transactions --account "Checking"      # Filter by account
pint transactions --from 2024-01-01         # From date
pint transactions --to 2024-12-31           # To date
pint transactions --search "Amazon"         # Search description
pint transactions --category Groceries      # Filter by category
pint transactions --uncategorized           # Show uncategorized only
pint transactions --limit 100               # Limit results (default: 50)
```

### Categorization

| Command | Description |
|---------|-------------|
| `pint import-rules` | Load/reload rules from config file |
| `pint categorize auto` | Auto-categorize using merchant rules |
| `pint categorize set <tx-id> <category>` | Manually set category |
| `pint categorize learn <tx-id> <category> <pattern>` | Set category and create rule |

#### How Categorization Works

1. Rules map patterns to categories (case-insensitive; default is substring match)
2. Pattern "AMAZON" matches "AMAZON.COM", "AMZN MKTP", "AMAZON PRIME"
3. Longer patterns take precedence over shorter ones
4. Rules are stored in `~/.local/share/pint/rules.toml`
5. Optional `match` modes exist for safer matching (e.g. `token` avoids matching "IRS" inside "FIRST")

Example workflow for new merchants:

```bash
# See uncategorized transactions
pint transactions -u

# Found "JOES COFFEE SHOP" - categorize and learn
pint categorize learn abc123 "Coffee & Cafes" "JOES COFFEE"

# Future transactions matching "JOES COFFEE" will auto-categorize
```

## Data Location

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/pint/` |
| macOS | `~/Library/Application Support/pint/` |

Contents:
- `pint.db` - SQLite database
- `rules.toml` - Merchant categorization rules (editable)

## Default Categories

The default rules file includes 377 patterns across 25 categories:

- Cash
- Charity
- Childcare
- Coffee & Cafes
- Education
- Entertainment
- Gas & Auto
- Government
- Groceries
- Health
- Home Services
- Income
- Insurance
- Investments
- Personal Care
- Pets
- Restaurants
- Shipping
- Shopping
- Subscriptions
- Taxes
- Transfers
- Transportation
- Travel
- Utilities

Edit `~/.local/share/pint/rules.toml` to customize, then run `pint import-rules`.

## Current Status

### Working

- SimpleFIN integration (token exchange, account/transaction sync)
- Historical backfill (fetches in 60-day chunks, up to ~2 years)
- SQLite storage with proper schema
- Transaction listing with filters
- Merchant-based auto-categorization
- Manual categorization with rule learning
- 377 pre-defined rules for common US merchants

### Not Yet Implemented

- [ ] Transfer linking (mark two transactions as a transfer pair)
- [ ] Transaction notes/tags
- [ ] Spending reports and summaries
- [ ] Data export (CSV, JSON)
- [ ] TUI interface (planned with Ratatui)
- [ ] Multi-currency support
- [ ] Scheduled/recurring transaction detection
- [ ] Budget tracking

## SimpleFIN Notes

- Rate limit: 24 requests per day
- Date range: 60 days max per request
- Setup token can only be exchanged once
- Access URL is stored in the database
- Backfill makes up to 12 requests (60 days each) to fetch ~2 years of history
- Backfill stops early if no new transactions are found (institution limit)

## License

MIT
