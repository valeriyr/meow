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
            OutputFormatter::Json => {
                format!("{}", serde_json::to_string_pretty(output)?)
            }
            OutputFormatter::Table => {
                let json = json![output];

                let mut table = json_to_table(&json);

                table.with(tabled::settings::Style::rounded().horizontals([]));
                table.array_orientation(Orientation::Column);

                format!("{}", table)
            }
        })
    }
}
