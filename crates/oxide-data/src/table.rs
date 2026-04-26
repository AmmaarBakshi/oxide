use crate::value::Value;

#[derive(Debug, Clone)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    pub fn new(headers: Vec<String>) -> Self {
        Self {
            headers,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<Value>) {
        // Ensure the row length matches the header length before adding
        if row.len() == self.headers.len() {
            self.rows.push(row);
        }
    }
}