//! Small value types and helpers shared by the invoice and project-planner
//! forms.

use std::fmt;
use std::str::FromStr;

/// A minimal calendar date that parses/formats as `YYYY-MM-DD`, so it can back a
/// `<input type="date">` through the typed date binding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DateYmd {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl DateYmd {
    pub const fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }
}

impl fmt::Display for DateYmd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

impl FromStr for DateYmd {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('-');
        let year = parts.next().and_then(|p| p.parse().ok());
        let month = parts.next().and_then(|p| p.parse().ok());
        let day = parts.next().and_then(|p| p.parse().ok());
        match (year, month, day) {
            (Some(year), Some(month @ 1..=12), Some(day @ 1..=31)) => Ok(Self { year, month, day }),
            _ => Err("Use the format YYYY-MM-DD.".to_string()),
        }
    }
}

pub fn money(cents: u32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// Parse a dollar string like `48.00` into integer cents.
pub fn parse_dollars_to_cents(raw: &str) -> Result<u32, String> {
    let value: f64 = raw
        .trim()
        .parse()
        .map_err(|_| "Enter a dollar amount.".to_string())?;
    if value < 0.0 {
        return Err("Amount cannot be negative.".to_string());
    }
    Ok((value * 100.0).round() as u32)
}

pub fn cents_to_dollars(cents: u32) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}
