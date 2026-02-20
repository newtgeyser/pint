---
description: Scrape rewards points from bank websites using Playwright
disable-model-invocation: true
allowed-tools: mcp__playwright__browser_navigate, mcp__playwright__browser_snapshot, mcp__playwright__browser_click, mcp__playwright__browser_fill_form, mcp__playwright__browser_type, mcp__playwright__browser_wait_for, mcp__playwright__browser_tabs, mcp__playwright__browser_take_screenshot, mcp__playwright__browser_evaluate, mcp__playwright__browser_handle_dialog, mcp__playwright__browser_press_key, Read, Bash
---

# Scrape Rewards Points

Scrape current rewards points balances from bank websites, then store them using `pint points set`.

## Step 1: Load Configuration

Read the file `~/.config/pint/secrets.env` using the Read tool. Parse it as dotenv format: each line is `KEY=VALUE` or `KEY="VALUE"`. Strip any surrounding double quotes from values.

Group the credentials by bank. Each variable follows the pattern `BANKNAME_USER` / `BANKNAME_PASS` for single-account banks, or `BANKNAME_USER_N` / `BANKNAME_PASS_N` (N = 1, 2, 3, ...) for banks with multiple logins. Extract the distinct bank names and build a list of (bank, account-index, username, password) tuples to process.

Only scrape banks/accounts that have credentials defined. If a bank has no credentials, skip it. If an account is missing its password, skip it and tell the user.

## Step 2: For Each Bank Account

Process each (bank, account) sequentially. For every account:

### 2a. Navigate to the bank's login page

Look up the bank's login URL. If you don't know it, search the web for "`<bank name>` online banking sign in". Navigate there.

### 2b. Log in

Take a snapshot. Find the username and password fields and fill them in. Submit the form.

**Bank-specific login hints** (non-exhaustive — adapt to what the snapshot shows):
- Some banks render their login form inside an iframe. If you don't see input fields in the snapshot, look for iframe content.
- Some banks use a two-step login (username on one page, password on the next).

### 2c. Handle MFA

If the bank presents a verification challenge (SMS code, authenticator app, security questions, etc.):
- Ask the user which method they prefer if there are options.
- Ask the user for the code and enter it.
- Wait for verification to complete.

### 2d. Scrape rewards points

Once logged in, take a snapshot of the dashboard/account summary. Look for any rewards points, miles, or cash back balances.

- Some banks show a combined total across cards. Look for a way to see per-card breakdowns (e.g., a "View Rewards" link or account picker dialog).
- If the account has no rewards program or shows 0 points, that's fine — record 0.

For each rewards balance found, run:
```
pint points set "<BankName> <card-name> (<last4>)" <points> -n "<rewards-currency>"
```

Where:
- `<BankName>` is the bank name (e.g., the name from the credentials)
- `<card-name>` is the card name as shown on the dashboard (e.g., "Sapphire Preferred", "Venture X")
- `<last4>` is the last 4 digits of the card number, as shown on the dashboard (e.g., "...1234" → "1234")
- `<points>` is the numeric balance (strip commas)
- `<rewards-currency>` is the type of reward (e.g., "Miles", "Ultimate Rewards", "Cash Back" — use whatever the bank calls it)

### 2e. Log out

Find and click the sign out / log out option before moving to the next account.

## Step 3: Show Results

After scraping all accounts, run:
```
pint points
```

Display the output to show the user the updated balances.

## General Rules

- Use `browser_snapshot` for all navigation decisions — find elements by their visible text labels, never hardcode CSS selectors or element IDs.
- Use `browser_take_screenshot` only when something goes wrong, to help debug failures.
- Bank sites are slow. After any navigation or click, use `browser_wait_for` to wait for expected content before taking a snapshot.
- If you land on a page and are already logged in, skip the login steps and go straight to scraping.
- If a login fails, take a screenshot, report the error to the user, and move on to the next account. Never retry a failed login more than once — repeated failures can trigger security lockouts.
- Passwords may contain special characters. When passing values to shell commands, use proper quoting.
