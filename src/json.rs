//! JSON operations are independent of the UI so invalid edits never destroy text.
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

pub fn validate(source: &str) -> Result<(), JsonError> {
    parse(source).map(|_| ())
}

pub fn format(source: &str) -> Result<String, JsonError> {
    let value = parse(source)?;
    Ok(serde_json::to_string_pretty(&value).expect("JSON values can be serialized"))
}

fn parse(source: &str) -> Result<Value, JsonError> {
    serde_json::from_str(source).map_err(|error| JsonError {
        message: error.to_string(),
        line: error.line(),
        column: error.column(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_preserves_order_precision_and_unicode() {
        let text = r#"{"z":123456789012345678901234567890,"a":"你好","nested":[true,null,{"n":1.234567890123456789}]}"#;
        let formatted = format(text).unwrap();
        assert!(formatted.starts_with("{\n  \"z\": 123456789012345678901234567890,"));
        assert!(formatted.contains("1.234567890123456789"));
        assert!(formatted.contains("你好"));
        assert_eq!(parse(text).unwrap(), parse(&formatted).unwrap());
    }

    #[test]
    fn validation_reports_error_location() {
        let error = validate("{\n  \"a\": ,\n  \"b\": 2\n}").unwrap_err();
        assert_eq!((error.line, error.column), (2, 8));
    }

    #[test]
    fn rejects_empty_trailing_data_and_incomplete_documents() {
        for source in ["", "{} {}", "[1,]", "{\"a\":", "{\"a\": NaN}"] {
            assert!(validate(source).is_err(), "{source}");
            assert!(format(source).is_err(), "{source}");
        }
        for source in ["null", "42", "\"text\"", "[]", "{}"] {
            assert!(validate(source).is_ok());
        }
    }
}
