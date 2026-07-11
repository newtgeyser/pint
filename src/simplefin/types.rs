use anyhow::{Context, Result};
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
    pub holdings: Vec<Holding>,
    #[serde(default)]
    pub extra: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Holding {
    pub id: String,
    pub created: Option<i64>,
    pub currency: Option<String>,
    #[serde(rename = "cost_basis")]
    pub cost_basis: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "market_value")]
    pub market_value: Option<String>,
    #[serde(rename = "purchase_price")]
    pub purchase_price: Option<String>,
    pub shares: Option<String>,
    pub symbol: Option<String>,
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
        self.org
            .as_ref()
            .and_then(|o| o.name.as_deref().or(o.domain.as_deref()))
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

        // If account has holdings, it's likely a brokerage account
        if !self.holdings.is_empty() {
            return "brokerage";
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
    pub fn amount_cents(&self) -> Result<i64> {
        parse_amount_to_cents(&self.amount).context(format!(
            "Invalid transaction amount '{}' for transaction id {}",
            self.amount, self.id
        ))
    }
}

impl Holding {
    pub fn market_value_cents(&self) -> Option<i64> {
        self.market_value
            .as_ref()
            .and_then(|s| parse_amount_to_cents(s))
    }

    pub fn cost_basis_cents(&self) -> Option<i64> {
        self.cost_basis
            .as_ref()
            .and_then(|s| parse_amount_to_cents(s))
    }

    pub fn price_cents(&self) -> Option<i64> {
        let market_value = self.market_value_cents()?;
        let shares: f64 = self.shares.as_ref()?.parse().ok()?;
        if shares == 0.0 {
            return None;
        }
        Some((market_value as f64 / shares).round() as i64)
    }
}

fn parse_amount_to_cents(amount: &str) -> Option<i64> {
    let amount = amount.trim();
    if amount.is_empty() {
        return None;
    }

    let (sign, rest) = match amount.as_bytes().first() {
        Some(b'-') => (-1_i64, amount[1..].trim()),
        Some(b'+') => (1_i64, amount[1..].trim()),
        _ => (1_i64, amount),
    };

    if rest.is_empty() {
        return None;
    }

    let mut parts = rest.splitn(2, '.');
    let whole_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");

    let whole_part = if whole_part.is_empty() {
        "0"
    } else {
        whole_part
    };

    if !whole_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let whole: i64 = whole_part.parse().ok()?;

    let mut frac_iter = frac_part.chars();
    let d1 = frac_iter.next().unwrap_or('0').to_digit(10)? as i64;
    let d2 = frac_iter.next().unwrap_or('0').to_digit(10)? as i64;
    let d3 = frac_iter.next().unwrap_or('0').to_digit(10)? as i64;

    let mut cents = d1 * 10 + d2;
    let mut carry = 0_i64;
    if frac_part.len() > 2 && d3 >= 5 {
        cents += 1;
        if cents == 100 {
            cents = 0;
            carry = 1;
        }
    }

    let total = whole
        .checked_mul(100)?
        .checked_add(cents)?
        .checked_add(carry)?;

    if sign == 1 {
        Some(total)
    } else {
        total.checked_neg()
    }
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
        assert_eq!(parse_amount_to_cents("1.2"), Some(120));
        assert_eq!(parse_amount_to_cents(".5"), Some(50));
        assert_eq!(parse_amount_to_cents("12.345"), Some(1235));
        assert_eq!(parse_amount_to_cents("-12.345"), Some(-1235));
    }
}
