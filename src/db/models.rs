use rusqlite::Row;

#[derive(Debug, Clone)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub nickname: Option<String>,
    pub institution: Option<String>,
    pub account_type: String,
    pub balance: Option<i64>,
    pub balance_date: Option<i64>,
    pub currency: String,
    pub manual: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Account {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            nickname: row.get("nickname")?,
            institution: row.get("institution")?,
            account_type: row.get("account_type")?,
            balance: row.get("balance")?,
            balance_date: row.get("balance_date")?,
            currency: row.get("currency")?,
            manual: row.get::<_, i64>("manual")? != 0,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn balance_dollars(&self) -> Option<f64> {
        self.balance.map(|cents| cents as f64 / 100.0)
    }

    /// Returns the display name: nickname if set, otherwise name
    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub posted: i64,
    pub amount: i64,
    pub description: String,
    pub pending: bool,
    pub category_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Transaction {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            account_id: row.get("account_id")?,
            posted: row.get("posted")?,
            amount: row.get("amount")?,
            description: row.get("description")?,
            pending: row.get::<_, i64>("pending")? != 0,
            category_id: row.get("category_id")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn amount_dollars(&self) -> f64 {
        self.amount as f64 / 100.0
    }
}

#[derive(Debug, Clone)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: i64,
}

impl Category {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            parent_id: row.get("parent_id")?,
            created_at: row.get("created_at")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MerchantRule {
    pub pattern: String,
    pub category_id: i64,
    pub created_at: i64,
}

impl MerchantRule {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            pattern: row.get("pattern")?,
            category_id: row.get("category_id")?,
            created_at: row.get("created_at")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Holding {
    pub id: String,
    pub account_id: String,
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub shares: String,
    pub price: Option<i64>,
    pub cost_basis: Option<i64>,
    pub market_value: Option<i64>,
    pub currency: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Holding {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            account_id: row.get("account_id")?,
            symbol: row.get("symbol")?,
            description: row.get("description")?,
            shares: row.get("shares")?,
            price: row.get("price")?,
            cost_basis: row.get("cost_basis")?,
            market_value: row.get("market_value")?,
            currency: row.get("currency")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn price_dollars(&self) -> Option<f64> {
        self.price.map(|cents| cents as f64 / 100.0)
    }

    pub fn cost_basis_dollars(&self) -> Option<f64> {
        self.cost_basis.map(|cents| cents as f64 / 100.0)
    }

    pub fn market_value_dollars(&self) -> Option<f64> {
        self.market_value.map(|cents| cents as f64 / 100.0)
    }
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub asset_type: String,
    pub description: Option<String>,
    pub value: Option<i64>,
    pub cost_basis: Option<i64>,
    pub currency: String,
    pub acquired_date: Option<i64>,
    pub metadata: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Asset {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            asset_type: row.get("asset_type")?,
            description: row.get("description")?,
            value: row.get("value")?,
            cost_basis: row.get("cost_basis")?,
            currency: row.get("currency")?,
            acquired_date: row.get("acquired_date")?,
            metadata: row.get("metadata")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn value_dollars(&self) -> Option<f64> {
        self.value.map(|cents| cents as f64 / 100.0)
    }

    pub fn cost_basis_dollars(&self) -> Option<f64> {
        self.cost_basis.map(|cents| cents as f64 / 100.0)
    }
}
