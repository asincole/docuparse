mod completion;
mod document_info;
mod error;
mod info;
#[cfg(feature = "ocr")]
mod ocr;
mod pages;
mod render;
mod text;

use completion::Completion;
use info::Info;
#[cfg(feature = "ocr")]
use ocr::Ocr;
use render::Render;
use text::Text;
use usage_rs::{Cli, Subcommands};

/// PDF processing on the command line - text extraction, page rendering, and OCR.
#[derive(Cli)]
#[usage(bin = "docuparse-cli", version = env!("CARGO_PKG_VERSION"), completion)]
pub(crate) struct DocuparseCli {
    #[usage(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommands)]
#[usage(run)]
pub(crate) enum Commands {
    Info(Info),
    Text(Text),
    Render(Render),
    #[cfg(feature = "ocr")]
    Ocr(Ocr),
    Completion(Completion),
}
