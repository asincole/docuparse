use std::{
    error::Error,
    fs,
    io::{self, BufWriter, Write},
    path::PathBuf,
};

use docuparse::{PdfDocument, PdfOpenConfig, RenderConfig};
use image::codecs::png::PngEncoder;
use serde::Serialize;
use usage_rs::{Args, Run};

use super::{error::fail, pages::PageRange};

/// Render PDF pages to PNG images.
#[derive(Args)]
pub(crate) struct Render {
    /// Path to the PDF file
    path: String,

    /// Directory to write rendered pages into (created if missing).
    /// Required unless --stdout is set.
    #[usage(long)]
    out_dir: Option<PathBuf>,

    /// Write raw PNG bytes to stdout. Only valid for a single page.
    #[usage(long)]
    stdout: bool,

    /// Pages to render, e.g. "1,3,5-8" (1-based). Defaults to all pages.
    #[usage(long)]
    pages: Option<PageRange>,

    /// Render resolution in DPI
    #[usage(long, default = "150")]
    dpi: u32,

    /// Print a machine-readable JSON manifest instead of a summary
    #[usage(long)]
    json: bool,
}

#[derive(Serialize)]
struct WrittenPage {
    page: u32,
    path: String,
}

#[derive(Serialize)]
struct Manifest {
    written: Vec<WrittenPage>,
}

impl Run for Render {
    type Output = ();

    fn run(self) {
        self.execute().unwrap_or_else(|e| fail(e));
    }
}

impl Render {
    fn execute(self) -> Result<(), Box<dyn Error>> {
        if self.stdout && self.json {
            return Err("--stdout and --json cannot be combined - both write to stdout".into());
        }

        if !self.stdout && self.out_dir.is_none() {
            return Err("either --out-dir or --stdout is required".into());
        }

        let doc = PdfDocument::load(&self.path, &PdfOpenConfig::builder().build())?;

        let wanted = match &self.pages {
            Some(range) => range.resolve(doc.page_count())?,
            None => PageRange::all(doc.page_count()),
        };

        let config = RenderConfig::builder().dpi(self.dpi).build();

        if self.stdout {
            let [page] = wanted.as_slice() else {
                return Err(
                    "--stdout only supports a single page - narrow down with --pages".into(),
                );
            };

            let image = doc.render_page(*page, &config)?;
            let stdout = io::stdout();
            let mut output = BufWriter::new(stdout.lock());

            image.write_with_encoder(PngEncoder::new(&mut output))?;
            output.flush()?;

            return Ok(());
        }

        let Some(out_dir) = self.out_dir else {
            return Err("either --out-dir or --stdout is required".into());
        };

        fs::create_dir_all(&out_dir)?;

        let stdout = io::stdout();
        let mut output = BufWriter::new(stdout.lock());
        let mut manifest = Manifest {
            written: Vec::with_capacity(if self.json { wanted.len() } else { 0 }),
        };

        for page in wanted {
            let image = doc.render_page(page, &config)?;
            let page_number = page + 1;
            let out_path = out_dir.join(format!("page_{page_number:03}.png"));

            image.save(&out_path)?;

            if self.json {
                manifest.written.push(WrittenPage {
                    page: page_number,
                    path: out_path.display().to_string(),
                });
            } else {
                writeln!(output, "page {page_number:>4} -> {}", out_path.display(),)?;
            }
        }

        if self.json {
            serde_json::to_writer(&mut output, &manifest)?;
            writeln!(output)?;
        }

        output.flush()?;
        Ok(())
    }
}
