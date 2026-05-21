//! Output formatter that renders command results as JSON or a human-readable table.

use json_to_table::Orientation;
use json_to_table::json_to_table;
use serde::Serialize;
use serde_json::json;
use strum_macros::EnumString;

/// The output format used to print a command output.
#[derive(Clone, Copy, Debug, EnumString, strum_macros::Display, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum OutputFormatter {
    /// Prints a JSON output representation.
    Json,
    /// Prints an output representation as a table.
    Table,
}

impl OutputFormatter {
    /// Formats the command output.
    pub fn format(&self, output: &impl Serialize) -> Result<String, anyhow::Error> {
        Ok(match self {
            OutputFormatter::Json => serde_json::to_string_pretty(output)?,
            OutputFormatter::Table => {
                let json = json!(output);

                let mut builder = json_to_table(&json);
                builder.with(tabled::settings::Style::rounded().horizontals([]));
                builder.array_orientation(Orientation::Column);

                let mut table = builder.into_table();
                table.with(tabled::settings::Width::wrap(140));

                format!("{}", table)
            }
        })
    }
}
