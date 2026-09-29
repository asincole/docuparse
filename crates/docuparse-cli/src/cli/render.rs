use std::{
    io::{Cursor, Write},
    path::PathBuf,
};

use docuparse::{PdfDocument, PdfOpenConfig, RenderConfig};
use image::ImageFormat;
use usage_rs::{Args, Run};

use super::{error::fail, pages::PageRange};

/// Render PDF pages to PNG images.
#[derive(Args)]
pub(crate) struct Render {
    /// Path to the PDF file
    path: String,

    /// Directory to write rendered pages into (created if missing). Required unless --stdout is set.
    #[usage(long)]
    out_dir: Option<PathBuf>,

    /// Write raw PNG bytes to stdout instead of saving files. Only valid for a single page.
    #[usage(long)]
    stdout: bool,

    /// Pages to render, e.g. "1,3,5-8" (1-based). Defaults to all pages.
    #[usage(long)]
    pages: Option<PageRange>,

    /// Render resolution in DPI
    #[usage(long, default = "150")]
    dpi: u32,

    /// Print machine-readable JSON manifest instead of a summary
    #[usage(long)]
    json: bool,
}

impl Run for Render {
    type Output = ();

    fn run(self) {
        if self.stdout && self.json {
            fail("--stdout and --json cannot be combined - both write to stdout");
        }
        if !self.stdout && self.out_dir.is_none() {
            fail("either --out-dir or --stdout is required");
        }

        let doc = PdfDocument::load(&self.path, &PdfOpenConfig::builder().build())
            .unwrap_or_else(|e| fail(e));

        let wanted = match &self.pages {
            Some(range) => range.resolve(doc.page_count()).unwrap_or_else(|e| fail(e)),
            None => PageRange::all(doc.page_count()),
        };

        let config = RenderConfig::builder().dpi(self.dpi).build();

        if self.stdout {
            if wanted.len() != 1 {
                fail("--stdout only supports a single page - narrow down with --pages");
            }

            let image = doc.render_page(wanted[0], &config).unwrap_or_else(|e| fail(e));

            let mut buf = Vec::new();
            image
                .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                .unwrap_or_else(|e| fail(e));

            std::io::stdout().write_all(&buf).unwrap_or_else(|e| fail(e));
            return;
        }

        let out_dir = self.out_dir.as_ref().expect("checked above");
        std::fs::create_dir_all(out_dir).unwrap_or_else(|e| fail(e));

        let mut written = Vec::new();

        for page in wanted {
            let image = doc.render_page(page, &config).unwrap_or_else(|e| fail(e));
            let out_path = out_dir.join(format!("page_{:03}.png", page + 1));
            image.save(&out_path).unwrap_or_else(|e| fail(e));

            if self.json {
                written.push(serde_json::json!({
                    "page": page + 1,
                    "path": out_path.display().to_string(),
                }));
            } else {
                println!("page {:>4} -> {}", page + 1, out_path.display());
            }
        }

        if self.json {
            println!("{}", serde_json::json!({ "written": written }));
        }
    }
}
