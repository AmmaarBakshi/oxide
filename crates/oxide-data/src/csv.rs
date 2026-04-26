use crate::table::Table;
use crate::value::Value;

/// Takes raw CSV text and converts it into an Oxide Table
pub fn parse(raw_text: &str) -> Result<Table, String> {
    // 1. Initialize the CSV reader from the raw text
    let mut reader = csv::Reader::from_reader(raw_text.as_bytes());

    // 2. Extract the header row
    let headers: Vec<String> = match reader.headers() {
        Ok(h) => h.iter().map(|s| s.to_string()).collect(),
        Err(e) => return Err(format!("Failed to read CSV headers: {}", e)),
    };

    let mut table = Table::new(headers);

    // 3. Loop through every row of data
    for result in reader.records() {
        let record = match result {
            Ok(r) => r,
            Err(e) => return Err(format!("Failed to read CSV row: {}", e)),
        };

        let mut row = Vec::new();
        
        // 4. Smart Data Typing: Guess what type of data is in the cell
        for field in record.iter() {
            if field.is_empty() {
                row.push(Value::Null);
            } else if let Ok(i) = field.parse::<i64>() {
                row.push(Value::Int(i));
            } else if let Ok(f) = field.parse::<f64>() {
                row.push(Value::Float(f));
            } else if let Ok(b) = field.parse::<bool>() {
                row.push(Value::Bool(b));
            } else {
                row.push(Value::String(field.to_string())); // Default to text
            }
        }
        
        table.add_row(row);
    }

    Ok(table)
}