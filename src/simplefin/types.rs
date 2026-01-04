use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct AccountSet {
    pub errors: Vec<String>,
    pub accounts: Vec<Account>,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub currency: Option<String>,
    pub balance: String,
    #[serde(rename = "available-balance")]
    pub available_balance: Option<String>,
    #[serde(rename = "balance-date")]
    pub balance_date: i64,
    pub org: Option<Organization>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub extra: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Organization {
    pub domain: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "sfin-url")]
    pub sfin_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub posted: i64,
    pub amount: String,
    pub description: String,
    #[serde(default)]
    pub pending: bool,
}

impl Account {
    pub fn institution_name(&self) -> Option<&str> {
        self.org.as_ref().and_then(|o| o.name.as_deref().or(o.domain.as_deref()))
    }

    pub fn balance_cents(&self) -> Option<i64> {
        parse_amount_to_cents(&self.balance)
    }

    pub fn account_type(&self) -> &str {
        // Try to extract account type from extra field
        // Common keys: "type", "account_type", "accountType"
        if let Some(extra) = &self.extra {
            for key in ["type", "account_type", "accountType", "account-type"] {
                if let Some(Value::String(t)) = extra.get(key) {
                    return normalize_account_type(t);
                }
            }
        }
        "unknown"
    }
}

fn normalize_account_type(t: &str) -> &'static str {
    let lower = t.to_lowercase();
    match lower.as_str() {
        "checking" | "dda" => "checking",
        "savings" | "sda" => "savings",
        "credit" | "credit card" | "creditcard" | "cc" => "credit",
        "brokerage" | "investment" | "investments" => "brokerage",
        "retirement" | "ira" | "401k" | "roth" => "retirement",
        "loan" | "mortgage" => "loan",
        "money market" | "mma" => "money market",
        _ => "unknown",
    }
}

impl Transaction {
    pub fn amount_cents(&self) -> i64 {
        parse_amount_to_cents(&self.amount).unwrap_or(0)
    }
}

fn parse_amount_to_cents(amount: &str) -> Option<i64> {
    let amount = amount.trim();
    if amount.is_empty() {
        return None;
    }

    let parsed: f64 = amount.parse().ok()?;
    Some((parsed * 100.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount_to_cents("100.23"), Some(10023));
        assert_eq!(parse_amount_to_cents("-33.50"), Some(-3350));
        assert_eq!(parse_amount_to_cents("0"), Some(0));
        assert_eq!(parse_amount_to_cents(""), None);
    }
}
