use serde::{Deserialize, Serialize};

/// Subreddit metadata used by the right-hand About panel.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Subreddit {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub subscribers: u64,
    #[serde(default, alias = "active")]
    pub active_users: u64,
    #[serde(default)]
    pub over18: bool,
    #[serde(default)]
    pub created_utc: i64,
}

impl Subreddit {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            title: name.clone(),
            name,
            description: String::new(),
            subscribers: 0,
            active_users: 0,
            over18: false,
            created_utc: 0,
        }
    }

    pub fn display_name(&self) -> String {
        format!("r/{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_has_prefix() {
        assert_eq!(Subreddit::new("rust").display_name(), "r/rust");
    }
}
