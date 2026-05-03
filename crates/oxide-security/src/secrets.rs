pub struct SecretStore {
    keys: Vec<String>,
}

impl SecretStore {
    pub fn new() -> Self {
        Self {
            keys: vec!["PASSWORD".to_string(), "TOKEN".to_string(), "SECRET".to_string()],
        }
    }

    pub fn mask(&self, key: &str, value: &str) -> String {
        if self.keys.iter().any(|k| key.contains(k)) {
            return "******* (PROTECTED)".to_string();
        }
        value.to_string()
    }
}