use std::{ops::Range, str::FromStr};

/// A 1-based, comma-separated list of pages and ranges, e.g. "1,3,5-8".
/// Stored as sorted, disjoint, 0-based half-open ranges.
#[derive(Clone, Debug)]
pub(crate) struct PageRange(Vec<Range<u32>>);

impl PageRange {
    /// Validates the ranges and returns sorted, unique 0-based indices.
    pub(crate) fn resolve(&self, page_count: u32) -> Result<Vec<u32>, String> {
        if let Some(range) = self.0.iter().find(|range| range.end > page_count) {
            let page = range.start.max(page_count) + 1;

            return Err(format!(
                "page {page} out of range (document has {page_count} pages)"
            ));
        }

        Ok(self.0.iter().cloned().flatten().collect())
    }

    /// 0-based indices for every page, used when `--pages` is omitted.
    pub(crate) fn all(page_count: u32) -> Vec<u32> {
        (0..page_count).collect()
    }
}

impl FromStr for PageRange {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ranges = Vec::new();

        for part in s.split(',').map(str::trim) {
            if part.is_empty() {
                return Err("page list contains an empty entry".to_owned());
            }

            let range = match part.split_once('-') {
                Some((start, end)) => {
                    let start = start
                        .trim()
                        .parse::<u32>()
                        .map_err(|_| format!("invalid range: {part}"))?;
                    let end = end
                        .trim()
                        .parse::<u32>()
                        .map_err(|_| format!("invalid range: {part}"))?;

                    if start == 0 || start > end {
                        return Err(format!("invalid range: {part}"));
                    }

                    (start - 1)..end
                }
                None => {
                    let page = part
                        .parse::<u32>()
                        .map_err(|_| format!("invalid page: {part}"))?;

                    if page == 0 {
                        return Err(format!("page numbers are 1-based: {part}"));
                    }

                    (page - 1)..page
                }
            };

            ranges.push(range);
        }

        ranges.sort_unstable_by_key(|range| range.start);

        // dedup_by passes the later element first and removes it on a match.
        ranges.dedup_by(|next, previous| {
            if next.start <= previous.end {
                previous.end = previous.end.max(next.end);
                true
            } else {
                false
            }
        });

        Ok(Self(ranges))
    }
}
