# Rewards Points Tracking Plan (saved for later implementation)

See the full plan at: /home/mriou/.claude/plans/clever-wandering-hamster.md

Summary: Add `rewards_points` table, `RewardPoints` model, and `pint points` CLI command (list/set/add/remove). Follows the existing `assets` pattern. No new dependencies. 8 files to modify/create.

Key: The `set` command does an idempotent upsert — this is what Claude calls after Playwright scraping to store balances.
