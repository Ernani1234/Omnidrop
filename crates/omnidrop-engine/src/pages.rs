//! Page selections such as `1-3, 5, 8-` (1-based, inclusive).

use crate::error::{EngineError, EngineResult, ErrorCode};
use std::ops::RangeInclusive;

/// Parses a page selection against a document with `page_count` pages.
///
/// Empty input (or `all`) selects every page as one range. An open range like
/// `8-` runs to the last page.
#[allow(clippy::single_range_in_vec_init)]
pub fn parse_ranges(spec: &str, page_count: u32) -> EngineResult<Vec<RangeInclusive<u32>>> {
    if page_count == 0 {
        return Err(invalid("document has no pages"));
    }
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("all") {
        return Ok(vec![1..=page_count]);
    }
    let mut ranges = Vec::new();
    for part in spec.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let range = match part.split_once('-') {
            Some((start, end)) => {
                let start = if start.trim().is_empty() {
                    1
                } else {
                    parse_page(start)?
                };
                let end = if end.trim().is_empty() {
                    page_count
                } else {
                    parse_page(end)?
                };
                if start > end {
                    return Err(invalid(format!("range {part} is reversed")));
                }
                start..=end
            }
            None => {
                let page = parse_page(part)?;
                page..=page
            }
        };
        if *range.end() > page_count {
            return Err(invalid(format!(
                "page {} is beyond the last page ({page_count})",
                range.end()
            )));
        }
        ranges.push(range);
    }
    if ranges.is_empty() {
        return Err(invalid("no pages selected"));
    }
    Ok(ranges)
}

/// Sorted, de-duplicated list of selected pages.
pub fn selected_pages(spec: &str, page_count: u32) -> EngineResult<Vec<u32>> {
    let mut pages: Vec<u32> = parse_ranges(spec, page_count)?
        .into_iter()
        .flatten()
        .collect();
    pages.sort_unstable();
    pages.dedup();
    Ok(pages)
}

fn parse_page(raw: &str) -> EngineResult<u32> {
    let page: u32 = raw
        .trim()
        .parse()
        .map_err(|_| invalid(format!("'{}' is not a page number", raw.trim())))?;
    if page == 0 {
        return Err(invalid("pages start at 1"));
    }
    Ok(page)
}

fn invalid(detail: impl Into<String>) -> EngineError {
    EngineError::new(ErrorCode::InvalidPageRange, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_ranges() {
        assert_eq!(
            parse_ranges("1-3, 5, 8-", 10).unwrap(),
            vec![1..=3, 5..=5, 8..=10]
        );
        assert_eq!(parse_ranges("", 4).unwrap(), vec![1..=4]);
        assert_eq!(parse_ranges("all", 4).unwrap(), vec![1..=4]);
        assert_eq!(parse_ranges("-2", 4).unwrap(), vec![1..=2]);
    }

    #[test]
    fn rejects_invalid_ranges() {
        for bad in ["0", "5", "3-2", "a", "1-x", ",", "2-9"] {
            let err = parse_ranges(bad, 4).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidPageRange, "{bad}");
        }
        assert!(parse_ranges("1", 0).is_err());
    }

    #[test]
    fn selected_pages_are_unique_and_sorted() {
        assert_eq!(selected_pages("3,1-2,2", 5).unwrap(), vec![1, 2, 3]);
    }
}
