use std::str::FromStr;

/// A 1-based, comma-separated list of pages and ranges, e.g. "1,3,5-8".
/// Parsed into sorted, deduped 0-based indices.
#[derive(Clone, Debug)]
pub(crate) struct PageRange(Vec<u32>);

impl PageRange {
    /// Validates the parsed indices against the document's page count.
    pub(crate) fn resolve(&self, page_count: u32) -> Result<Vec<u32>, String> {
        for &p in &self.0 {
            if p >= page_count {
                return Err(format!(
                    "page {} out of range (document has {page_count} pages)",
                    p + 1
                ));
            }
        }
        Ok(self.0.clone())
    }

    /// 0-based indices for every page, used when `--pages` is omitted.
    pub(crate) fn all(page_count: u32) -> Vec<u32> {
        (0..page_count).collect()
    }
}

impl FromStr for PageRange {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut pages = Vec::new();

        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('-') {
                Some((start, end)) => {
                    let start: u32 = start.parse().map_err(|_| format!("invalid range: {part}"))?;
                    let end: u32 = end.parse().map_err(|_| format!("invalid range: {part}"))?;
                    if start == 0 || end == 0 || start > end {
                        return Err(format!("invalid range: {part}"));
                    }
                    pages.extend(start - 1..end);
                }
                None => {
                    let page: u32 = part.parse().map_err(|_| format!("invalid page: {part}"))?;
                    if page == 0 {
                        return Err(format!("page numbers are 1-based: {part}"));
                    }
                    pages.push(page - 1);
                }
            }
        }

        pages.sort_unstable();
        pages.dedup();
        Ok(PageRange(pages))
    }
}
