# Claude Code Development Notes

## Development Environment

**IMPORTANT**: Always use a separate data directory for development to avoid affecting production data:

```bash
# Use this for all development/testing commands:
pint --data-dir ./dev-data <command>

# Or set the environment variable:
export PINT_DATA_DIR=./dev-data
```

Never run destructive commands (like `rm` on database files) against the production path `~/.local/share/pint/`.

## Build & Test

```bash
cargo build
cargo test
cargo run -- --data-dir ./dev-data <command>
```

## Project Structure

- `src/main.rs` - CLI entry point (clap)
- `src/config.rs` - Data directory and path configuration
- `src/db/` - Database schema, models, queries
- `src/simplefin/` - SimpleFIN API client
- `src/commands/` - CLI command implementations
- `src/rules.rs` - Merchant rule loading and auto-categorization
- `default_rules.toml` - Default merchant categorization rules
