use anyhow::{Context, Result, bail};
use base64::prelude::*;
use reqwest::blocking::Client;
use std::time::Duration;

use super::types::AccountSet;

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

pub struct SimpleFin {
    client: Client,
    access_url: String,
}

impl SimpleFin {
    pub fn new(access_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self { client, access_url })
    }

    pub fn exchange_token(setup_token: &str) -> Result<String> {
        let setup_token = setup_token.trim();
        let claim_url = String::from_utf8(
            BASE64_STANDARD
                .decode(setup_token)
                .context("Invalid setup token: not valid base64")?,
        )
        .context("Invalid setup token: not valid UTF-8")?;

        let client = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .context("Failed to build HTTP client")?;
        let response = client
            .post(&claim_url)
            .send()
            .context("Failed to exchange setup token")?;

        if !response.status().is_success() {
            bail!(
                "Failed to exchange setup token: {} {}",
                response.status(),
                response.text().unwrap_or_default()
            );
        }

        let access_url = response.text().context("Failed to read access URL")?;
        Ok(access_url.trim().to_string())
    }

    pub fn fetch_accounts(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
    ) -> Result<AccountSet> {
        let mut url = format!("{}/accounts", self.access_url);

        let mut params = Vec::new();
        if let Some(start) = start_date {
            params.push(format!("start-date={}", start));
        }
        if let Some(end) = end_date {
            params.push(format!("end-date={}", end));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self
            .client
            .get(&url)
            .send()
            .context("Failed to fetch accounts")?;

        if !response.status().is_success() {
            bail!(
                "Failed to fetch accounts: {} {}",
                response.status(),
                response.text().unwrap_or_default()
            );
        }

        let text = response.text().context("Failed to read response")?;

        let account_set: AccountSet =
            serde_json::from_str(&text).context("Failed to parse account data")?;

        // Debug: print summary (not raw response, which contains sensitive data)
        if std::env::var("PINT_DEBUG").is_ok() {
            eprintln!("=== SIMPLEFIN RESPONSE SUMMARY ===");
            for account in &account_set.accounts {
                eprintln!(
                    "  {} — {} transactions, {} holdings",
                    account.name,
                    account.transactions.len(),
                    account.holdings.len()
                );
            }
            if !account_set.errors.is_empty() {
                eprintln!("  Errors: {:?}", account_set.errors);
            }
            eprintln!("=== END SUMMARY ===");
        }

        Ok(account_set)
    }
}
