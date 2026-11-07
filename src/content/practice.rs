// SPDX-License-Identifier: MIT OR Apache-2.0

//! The structure of the facts about the practice in `content/site.toml`.

use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

/// A phone number in display and in `tel:` form.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PhoneNumber {
    /// The number as printed, e.g., `03721 39040`.
    pub(crate) display: String,
    /// The number in international format for `tel:` links and schema.org.
    pub(crate) international: String,
}

/// The contact data and location of the practice.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Practice {
    /// The name of the website and the practice.
    pub(crate) site_name: String,
    /// The tagline below the site name.
    pub(crate) tagline: String,
    /// The name of the doctor with academic degree.
    pub(crate) doctor: String,
    /// The street and house number.
    pub(crate) street: String,
    /// The postal code.
    pub(crate) postal_code: String,
    /// The town as written on the website.
    pub(crate) locality: String,
    /// The federal state.
    pub(crate) region: String,
    /// The email address.
    pub(crate) email: String,
    /// The landline of the practice.
    pub(crate) phone: PhoneNumber,
    /// The fax number.
    pub(crate) fax: PhoneNumber,
    /// The mobile number.
    pub(crate) mobile: PhoneNumber,
    /// The latitude of the entrance, from the Google Maps link.
    pub(crate) latitude: f64,
    /// The longitude of the entrance, from the Google Maps link.
    pub(crate) longitude: f64,
    /// The link to the practice on Google Maps.
    pub(crate) maps_url: String,
}

/// A time range on one day, e.g., 08:00 to 12:00.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TimeRange {
    /// The opening time as `HH:MM`.
    pub(crate) opens: String,
    /// The closing time as `HH:MM`.
    pub(crate) closes: String,
}

impl Display for TimeRange {
    /// Formats the range as on the previous website, e.g., `08:00-12:00 Uhr`.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{} Uhr", self.opens, self.closes)
    }
}

/// The opening hours of one weekday.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpeningDay {
    /// The German name of the day.
    pub(crate) name: String,
    /// The schema.org `DayOfWeek` value.
    pub(crate) schema_day: String,
    /// The morning hours, if any.
    pub(crate) morning: Option<TimeRange>,
    /// The afternoon hours, if any.
    pub(crate) afternoon: Option<TimeRange>,
}

impl OpeningDay {
    /// Returns all time ranges of the day.
    pub(crate) fn ranges(&self) -> impl Iterator<Item = &TimeRange> {
        self.morning.iter().chain(&self.afternoon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::site::SITE;

    /// The display format is the one of the previous website.
    #[test]
    fn time_range_uses_german_format() {
        let range = TimeRange {
            opens: "08:00".to_string(),
            closes: "12:00".to_string(),
        };

        assert_eq!(range.to_string(), "08:00-12:00 Uhr");
    }

    /// Monday has two ranges, Wednesday only the afternoon.
    #[test]
    fn ranges_lists_morning_and_afternoon() {
        let monday: Vec<_> = SITE.opening_hours[0]
            .ranges()
            .map(|r| r.to_string())
            .collect();
        let wednesday: Vec<_> = SITE.opening_hours[2]
            .ranges()
            .map(|r| r.to_string())
            .collect();

        assert_eq!(monday, ["08:00-12:00 Uhr", "16:00-18:00 Uhr"]);
        assert_eq!(wednesday, ["15:00-19:00 Uhr"]);
    }

    /// The `tel:` numbers must be the same numbers as the displayed ones.
    #[test]
    fn international_numbers_match_display_numbers() {
        let practice = &SITE.practice;
        for number in [&practice.phone, &practice.fax, &practice.mobile] {
            let national = format!("0{}", &number.international[3..]);

            assert_eq!(number.display.replace(' ', ""), national);
        }
    }
}
