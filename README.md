# Pint

A personal finance manager that imports data from [SimpleFIN](https://www.simplefin.org/) and stores it in a local SQLite database. Built to be slim, durable, and fully under your control—your data stays on your machine.

## Why Pint?

Mint-style financial services get bought, discontinue products and sell your data. Pint exists so you are not the product.
- Imports transactions from bank accounts without storing credentials
- Tracks holdings, assets, and reward points for a full net worth picture
- Keeps all data locally (SQLite)
- Allows enrichment (categories, rules, recurring detection)
- Interactive TUI for browsing everything at a glance

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
# 1. Initialize the database and configure SimpleFIN access
pint setup

# 2. Fetch transactions (last 30 days by default)
pint sync

# 3. Auto-categorize transactions using rules
pint categorize auto

# 4. View your data
pint accounts
pint transactions
pint tui
```

## Commands

### Setup & Sync

| Command | Description |
|---------|-------------|
| `pint setup` | Initialize database and configure SimpleFIN access |
| `pint sync [--days N]` | Fetch transactions (default: 30, max: 60) |
| `pint sync --backfill` | Fetch historical transactions (up to ~2 years) |
| `pint tui` | Launch interactive terminal UI |

### Accounts

| Command | Description |
|---------|-------------|
| `pint accounts` | List accounts with balances |
| `pint accounts add <name>` | Add a manual account |
| `pint accounts remove <id>` | Remove an account and its transactions/holdings |
| `pint accounts set-type <id> <type>` | Set account type (cash, credit, brokerage, retirement) |
| `pint accounts rename <id> <nickname>` | Set or clear an account nickname |

### Transactions

| Command | Description |
|---------|-------------|
| `pint transactions` | List transactions with filters |
| `pint recurring` | Show detected recurring transactions |
| `pint review` | Group uncategorized transactions by merchant |
| `pint history` | Show daily net-worth snapshots |
| `pint health` | Report stale and incomplete financial data |

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

#### Uncategorized Review

```bash
pint review
pint review categorize "ACME COFFEE" "Restaurants" --rule
pint review categorize "UTILITY CO" "Utilities" --rule --pattern "UTILITY CO"
```

Review groups repeated uncategorized descriptions into merchant-level work items. Categorizing a group updates all matching uncategorized transactions atomically and can create a rule for future imports.

### Categorization

| Command | Description |
|---------|-------------|
| `pint rules` | List categorization rules |
| `pint rules categories` | List all categories |
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

### Holdings & Assets

| Command | Description |
|---------|-------------|
| `pint holdings` | List investment holdings |
| `pint holdings add <account> <symbol> <shares> <price>` | Add a holding |
| `pint holdings update <id> --price <price>` | Update a holding |
| `pint holdings update-prices` | Update prices from Yahoo Finance |
| `pint holdings import <csv>` | Import holdings from CSV |
| `pint holdings remove <id>` | Remove a holding |
| `pint assets` | List manual assets (real estate, vehicles, etc.) |
| `pint assets add <name> <value> --type <type>` | Add an asset |
| `pint assets update <id> --value <value>` | Update an asset value |
| `pint assets remove <id>` | Remove an asset |

### Reward Points

| Command | Description |
|---------|-------------|
| `pint points` | List all reward point balances |
| `pint points set <program> <points> -n <note>` | Set points for a program (upsert) |
| `pint points add <program> <points> -n <note>` | Add a new program |
| `pint points remove <id>` | Remove a program |

### Reimbursable Expenses

Track expenses that should be paid back by an entity (employer, your own LLC, a friend, etc.). Useful for separating personal-card spending that you'll get reimbursed for from spending that's actually yours.

| Command | Description |
|---------|-------------|
| `pint reimbursers` | List reimbursing entities |
| `pint reimbursers add <name>` | Create an entity (e.g., "Employer", "My LLC") |
| `pint reimbursers remove <name>` | Delete an entity (must have no transactions) |
| `pint reimburse <tx-id> <entity>` | Mark a transaction as reimbursable by `<entity>` |
| `pint reimburse <tx-id> --paid` | Mark a reimbursable transaction as paid back |
| `pint reimburse <tx-id> --clear` | Remove the reimbursable marker |
| `pint reimbursable` | List all reimbursable transactions |
| `pint reimbursable --pending` | Only outstanding (not yet repaid) |
| `pint reimbursable --paid` | Only already reimbursed |
| `pint reimbursable --entity <name>` | Filter by entity |
| `pint reimbursements aging` | Show outstanding balances by age and entity |
| `pint reimbursements payment <entity> <amount>` | Record a reimbursement payment |

Payments can be partially or fully allocated to multiple expenses:

```bash
pint reimbursements payment Employer 250.00 \
  --date 2026-07-10 \
  --reference ACH-123 \
  --allocate tx-id-1=100.00 \
  --allocate tx-id-2=150.00
```

In the TUI, on the Transactions view: press `r` to mark/clear reimbursable on the selected row, and `p` to toggle paid/pending status. Reimbursable rows show a colored badge: yellow `[E:Employer]` for pending, green `[$:Employer]` for paid.

## Scraping Reward Points

Pint includes a [Claude Code](https://docs.anthropic.com/en/docs/claude-code) slash command (`/scrape-points`) that uses Playwright to log into bank websites, read reward point balances, and store them via `pint points set`. Claude adaptively reads page snapshots instead of brittle CSS selectors, so bank site layout changes don't break things. MFA challenges are handled interactively in the conversation.

### Prerequisites

1. **Claude Code** installed and working
2. **Chromium** running with remote debugging enabled:
   ```bash
   chromium --remote-debugging-port=9222
   ```
3. **Playwright MCP** configured in `.mcp.json` at the project root:
   ```json
   {
     "mcpServers": {
       "playwright": {
         "command": "npx",
         "args": [
           "@playwright/mcp@latest",
           "--cdp-endpoint", "http://localhost:9222"
         ]
       }
     }
   }
   ```
4. **Credentials file** at `~/.config/pint/secrets.env`:
   ```env
   # Single-account bank
   BANKNAME_USER=myusername
   BANKNAME_PASS="myp@ssword"

   # Bank with multiple logins
   CHASE_USER_1=login1
   CHASE_PASS_1="pass1"
   CHASE_USER_2=login2
   CHASE_PASS_2="pass2"
   ```
   The naming convention is `BANKNAME_USER` / `BANKNAME_PASS` for single accounts, or `BANKNAME_USER_N` / `BANKNAME_PASS_N` for banks where you have multiple logins.

### Usage

In a Claude Code session:

```
/scrape-points
```

Claude will read your credentials, log into each bank sequentially, scrape reward balances, and store them. If MFA is triggered, it will ask you for the code in the conversation.

## TUI

`pint tui` launches an interactive terminal interface with:

- **Summary** — net worth overview with account type breakdown, holdings, assets, and reward points
- **Accounts** — balances across all accounts
- **Transactions** — searchable, filterable transaction list
- **Recurring** — detected recurring expenses with frequency and average amounts

Recurring forecasts support weekly, biweekly, monthly, bi-monthly, quarterly, and annual schedules, including expected dates, overdue/inactive status, amount variance, and normalized monthly commitment.

### History and Data Health

Pint captures one net-worth snapshot per day after successful syncs. Run `pint history --capture` to create or refresh today's snapshot manually. The TUI summary shows changes against prior snapshots.

`pint health` reports stale balances, missing holding prices or cost bases, missing asset values, uncategorized transactions, and aged pending reimbursements. Reimbursable transactions are excluded from the TUI's personal-spending summary.

## Data Location

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/pint/` |
| macOS | `~/Library/Application Support/pint/` |

Contents:
- `pint.db` — SQLite database
- `rules.toml` — Merchant categorization rules (editable)

Override with `--data-dir <path>` or `PINT_DATA_DIR` environment variable.

## Security Considerations

Pint is fully local and its code is auditable. It relies on SimpleFIN to access financial institution data. For a typical US consumer "transaction aggregation" use case, SimpleFIN Bridge is secure enough if you already accept the risk profile of Plaid/MX-style aggregation:

- The protocol is designed around read-only access (you're granting a "window," not "control")
- Revocable access tokens (apps can be cut off), with alerts on new IP access
- No credential storage on their side (delegated to MX), with a published pentest summary

It is not secure enough if your requirement is "no third party should ever access my financial data" or if you treat transaction-level data as highly sensitive, because by design you are introducing third parties and a tokenized access surface.

### SimpleFIN Notes

- Rate limit: 24 requests per day
- Date range: 60 days max per request
- Setup token can only be exchanged once
- Backfill makes up to 12 requests (60 days each) to fetch ~2 years of history
- Backfill stops early if no new transactions are found

## License

MIT
