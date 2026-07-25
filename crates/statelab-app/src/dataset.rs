//! HTTP adapter for the Dataset Explorer (§6.2).
//!
//! The generators and the streaming primitive live in the shared
//! [`statelab_dataset`] crate (used by both this browser host and the Tauri
//! shell). This module only translates an HTTP request into a
//! [`DatasetSpec`] — no generation logic and no trajectory mathematics.

pub use statelab_dataset::{stream_dataset, DatasetSpec};

/// Parses a dataset spec from the request query string (and `body`, for CSV).
pub fn spec_from_request(query: &str, body: &str) -> Option<DatasetSpec> {
    let kind = crate::query_param(query, "type")?;
    match kind.as_str() {
        "range" => Some(DatasetSpec::Range {
            start: num(query, "start", 1),
            end: num(query, "end", 100),
        }),
        "random" => Some(DatasetSpec::Random {
            count: num(query, "count", 100),
            max: num::<u64>(query, "max", 10_000).max(1),
            seed: num(query, "seed", 42),
        }),
        "primes" => Some(DatasetSpec::Primes {
            count: num(query, "count", 100),
        }),
        "even" => Some(DatasetSpec::Even {
            start: num(query, "start", 1),
            end: num(query, "end", 100),
        }),
        "odd" => Some(DatasetSpec::Odd {
            start: num(query, "start", 1),
            end: num(query, "end", 100),
        }),
        "powers-of-two" => Some(DatasetSpec::PowersOfTwo {
            count: num::<u64>(query, "count", 32) as u32,
        }),
        "csv" => {
            // CSV values arrive in the request body (POST); fall back to a
            // `values` query param for convenience.
            let raw = if body.trim().is_empty() {
                crate::query_param(query, "values").unwrap_or_default()
            } else {
                body.to_string()
            };
            Some(DatasetSpec::csv_from_text(&raw))
        }
        _ => None,
    }
}

/// Parses a query param as a number, using `default` when absent or unparseable.
fn num<T: std::str::FromStr>(query: &str, key: &str, default: T) -> T {
    crate::query_param(query, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_generator_kind() {
        assert_eq!(
            spec_from_request("type=range&start=1&end=5", ""),
            Some(DatasetSpec::Range { start: 1, end: 5 })
        );
        assert_eq!(
            spec_from_request("type=powers-of-two&count=8", ""),
            Some(DatasetSpec::PowersOfTwo { count: 8 })
        );
        assert_eq!(
            spec_from_request("type=primes&count=5", ""),
            Some(DatasetSpec::Primes { count: 5 })
        );
        assert_eq!(
            spec_from_request("type=random&count=3&max=50&seed=7", ""),
            Some(DatasetSpec::Random {
                count: 3,
                max: 50,
                seed: 7
            })
        );
    }

    #[test]
    fn csv_reads_the_post_body() {
        assert_eq!(
            spec_from_request("type=csv", "3, 27, 97"),
            Some(DatasetSpec::Csv {
                values: vec!["3".into(), "27".into(), "97".into()]
            })
        );
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert_eq!(spec_from_request("type=nonsense", ""), None);
        assert_eq!(spec_from_request("", ""), None);
    }

    #[test]
    fn missing_params_fall_back_to_defaults() {
        assert_eq!(
            spec_from_request("type=range", ""),
            Some(DatasetSpec::Range { start: 1, end: 100 })
        );
    }
}
