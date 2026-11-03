//! Command input handling.
//!
//! Tauri reports argument deserialization errors as plain strings. To keep the
//! uniform error format (section 8) for *every* failure, commands receive their
//! payload as [`Input<T>`]. It deserializes the argument itself, runs the
//! type's [`Validate`] implementation and turns any failure into a
//! [`CommandError`] with code `INVALID_INPUT`.

use serde::de::DeserializeOwned;
use tauri::Runtime;
use tauri::ipc::{CommandArg, CommandItem, InvokeBody, InvokeError};

use crate::error::CommandError;

/// Semantic validation that runs after deserialization. Every command input
/// must implement it, so "Rust validates all input" is enforced by the type
/// system.
pub trait Validate {
    fn validate(&self) -> Result<(), CommandError>;
}

/// Request of commands that take no parameters: `{}`. Unknown fields are
/// rejected like everywhere else.
#[derive(Debug, Clone, Default, serde::Deserialize, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct EmptyRequest {}

impl Validate for EmptyRequest {
    fn validate(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

/// A deserialized and validated command argument.
#[derive(Debug)]
pub struct Input<T>(pub T);

impl<T> Input<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: DeserializeOwned + Validate> Input<T> {
    /// Parses and validates a raw JSON argument. `key` is the argument name.
    pub fn parse(key: &str, value: Option<&serde_json::Value>) -> Result<Self, CommandError> {
        let value = value.ok_or_else(|| CommandError::invalid_input(key, "missing"))?;
        // Requests are JSON objects. serde would also build a struct from an
        // array of its field values; that form is not part of the contract.
        if !value.is_object() {
            return Err(CommandError::invalid_input(key, "not an object"));
        }
        let parsed: T = serde_json::from_value(value.clone())
            .map_err(|error| CommandError::invalid_input(key, error.to_string()))?;
        parsed.validate()?;
        Ok(Input(parsed))
    }
}

impl<'de, R: Runtime, T: DeserializeOwned + Validate> CommandArg<'de, R> for Input<T> {
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let value = match command.message.payload() {
            InvokeBody::Json(serde_json::Value::Object(map)) => map.get(command.key),
            InvokeBody::Json(_) | InvokeBody::Raw(_) => None,
        };
        Input::parse(command.key, value).map_err(InvokeError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Sample {
        count: u8,
    }

    impl Validate for Sample {
        fn validate(&self) -> Result<(), CommandError> {
            if self.count == 0 {
                return Err(CommandError::invalid_input("count", "must be positive"));
            }
            Ok(())
        }
    }

    #[test]
    fn accepts_valid_input() {
        let parsed = Input::<Sample>::parse("request", Some(&json!({ "count": 3 }))).unwrap();
        assert_eq!(parsed.into_inner().count, 3);
    }

    #[test]
    fn rejects_missing_argument() {
        let error = Input::<Sample>::parse("request", None).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(error.details.unwrap()["reason"], "missing");
    }

    #[test]
    fn rejects_wrong_type_and_unknown_fields() {
        for value in [
            json!({ "count": "three" }),
            json!({ "count": 1, "path": "/etc" }),
            json!(42),
            // The array form serde would otherwise accept for structs.
            json!([3]),
        ] {
            let error = Input::<Sample>::parse("request", Some(&value)).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidInput);
            assert_eq!(error.details.as_ref().unwrap()["field"], "request");
        }
    }

    #[test]
    fn runs_semantic_validation() {
        let error = Input::<Sample>::parse("request", Some(&json!({ "count": 0 }))).unwrap_err();
        assert_eq!(error.details.unwrap()["field"], "count");
    }
}
