use std::{
    sync::{Arc, mpsc},
    thread,
};

use crate::{
    OcrError,
    ocr::{OcrBackend, OcrConfig, OcrPageResult},
    pdf_document::PdfDocument,
    render::RenderConfig,
};

/// Sequential: render-then-infer for a single page.
pub(crate) fn ocr_page_with<R>(
    page_index: u32,
    render: &R,
    backend: &dyn OcrBackend,
    ocr_config: &OcrConfig,
) -> Result<OcrPageResult, OcrError>
where
    R: Fn(u32) -> Result<image::DynamicImage, OcrError>,
{
    let image = render(page_index)?;
    backend.run(&image, page_index, ocr_config).map_err(|e| {
        OcrError::InferenceFailed {
            page: page_index,
            source: Arc::from(e),
        }
        .into()
    })
}

/// Sequential bulk: fails fast on first error.
#[cfg(not(feature = "parallel"))]
pub(crate) fn ocr_all_pages_seq<R>(
    page_count: u32,
    render: &R,
    backend: &dyn OcrBackend,
    ocr_config: &OcrConfig,
) -> Result<Vec<OcrPageResult>, OcrError>
where
    R: Fn(u32) -> Result<image::DynamicImage, OcrError>,
{
    (0..page_count)
        .map(|i| ocr_page_with(i, render, backend, ocr_config))
        .collect()
}

/// Chunked parallel bulk: bounded memory, parallel inference per chunk.
#[cfg(feature = "parallel")]
pub(crate) fn ocr_all_pages_chunked<R>(
    page_count: u32,
    render: &R,
    backend: &(dyn OcrBackend + Sync),
    ocr_config: &OcrConfig,
    chunk_size: usize,
) -> Result<Vec<OcrPageResult>, OcrError>
where
    R: Fn(u32) -> Result<image::DynamicImage, OcrError> + Sync,
{
    use rayon::prelude::*;

    let mut all_results = Vec::with_capacity(page_count as usize);

    for chunk_start in (0..page_count).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size as u32).min(page_count);

        // Sequential render - PDFium is not thread-safe for concurrent access.
        let chunk_images: Result<Vec<_>, _> = (chunk_start..chunk_end)
            .map(|i| render(i).map(|img| (i, img)))
            .collect();

        let chunk_results: Result<Vec<_>, _> = chunk_images?
            .into_par_iter()
            .map(|(i, image)| {
                backend.run(&image, i, ocr_config).map_err(|e| {
                    OcrError::from(OcrError::InferenceFailed {
                        page: i,
                        source: Arc::from(e),
                    })
                })
            })
            .collect();

        all_results.extend(chunk_results?);
    }

    Ok(all_results)
}

/// Pipelined: bounded channel between render thread and inference thread.
pub(crate) fn ocr_pipeline_with<R>(
    page_count: u32,
    render: R,
    backend: Arc<dyn OcrBackend>,
    ocr_config: OcrConfig,
    channel_capacity: usize,
) -> Result<Vec<OcrPageResult>, OcrError>
where
    R: Fn(u32) -> Result<image::DynamicImage, OcrError>,
{
    let (tx, rx) = mpsc::sync_channel::<(u32, image::DynamicImage)>(channel_capacity);

    let inference_handle: thread::JoinHandle<Vec<Result<OcrPageResult, String>>> =
        thread::spawn(move || {
            let mut results = Vec::with_capacity(page_count as usize);
            while let Ok((page_index, image)) = rx.recv() {
                let result = backend
                    .run(&image, page_index, &ocr_config)
                    .map_err(|e| e.to_string());
                results.push(result);
            }
            results
        });

    for page_index in 0..page_count {
        let image = render(page_index)?;
        if tx.send((page_index, image)).is_err() {
            return Err(OcrError::InferenceFailed {
                page: page_index,
                source: Arc::new(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "OCR inference thread terminated unexpectedly",
                )),
            });
        }
    }

    drop(tx);

    inference_handle
        .join()
        .map_err(|_| OcrError::InferenceFailed {
            page: page_count.saturating_sub(1),
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "OCR inference thread panicked",
            )),
        })?
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            r.map_err(|e| OcrError::InferenceFailed {
                page: i as u32,
                source: Arc::new(std::io::Error::new(std::io::ErrorKind::Other, e)),
            })
        })
        .collect()
}

// ── PdfDocument delegators ────────────────────────────────────────────────────
//
// Zero logic here. If these are wrong, the kernel functions above are wrong
impl PdfDocument {
    /// Run OCR on a pre-rendered image for the given page index.
    ///
    /// This is the **primitive** OCR operation. The caller controls rendering
    /// DPI and format - pass the output of [`PdfDocument::render_page`] directly.
    /// Accepting `&DynamicImage` keeps the trait boundary flexible: backends
    /// that prefer JPEG encoding (e.g. cloud APIs, `turbojpeg`-based writers)
    /// can encode directly without an intermediate `RgbImage` allocation.
    ///
    /// # Recommended DPI
    ///
    /// The default [`RenderConfig`] uses 150 DPI which is marginal for OCR.
    /// Use ≥ 200 DPI for reliable text extraction.
    pub fn ocr_image(
        &self,
        page_index: u32,
        image: &image::DynamicImage,
        config: &OcrConfig,
    ) -> Result<OcrPageResult, OcrError> {
        let backend = self.require_ocr_backend()?;
        backend.run(image, page_index, config).map_err(|e| {
            OcrError::InferenceFailed {
                page: page_index,
                source: Arc::from(e),
            }
            .into()
        })
    }

    /// Render a page then immediately run OCR on the result.
    ///
    /// Use [`PdfDocument::ocr_image`] directly if you have already rendered the page for
    /// another purpose - this avoids the redundant pixel-buffer allocation.
    pub fn ocr_page(
        &self,
        page_index: u32,
        render_config: &RenderConfig,
        ocr_config: &OcrConfig,
    ) -> Result<OcrPageResult, OcrError> {
        let backend = self.require_ocr_backend()?;
        ocr_page_with(
            page_index,
            &|i| {
                self.render_page(i, render_config)
                    .map_err(|e| OcrError::RenderFailed {
                        page: i,
                        source: Arc::new(e),
                    })
            },
            backend,
            ocr_config,
        )
    }

    /// OCR every page sequentially. Fails fast on the first error.
    ///
    /// Enable the `parallel` feature for an identical-signature parallel
    /// implementation backed by rayon.
    #[cfg(not(feature = "parallel"))]
    pub fn ocr_all_pages(
        &self,
        render_config: &RenderConfig,
        ocr_config: &OcrConfig,
    ) -> Result<Vec<OcrPageResult>, OcrError> {
        let backend = self.require_ocr_backend()?;
        ocr_all_pages_seq(
            self.page_count(),
            &|i| {
                self.render_page(i, render_config)
                    .map_err(|e| OcrError::RenderFailed {
                        page: i,
                        source: Arc::new(e),
                    })
            },
            backend,
            ocr_config,
        )
    }

    /// OCR every page in parallel using rayon's work-stealing thread pool.
    ///
    /// Pages share the `Arc<dyn OcrBackend>` - the backend must be `Send + Sync`,
    /// enforced by the trait bound. Inference calls are dispatched in chunks of 8
    /// pages, rendering sequentially (PDFium is not thread-safe) then inferring
    /// in parallel.
    ///
    /// # Async callers
    ///
    /// This method is synchronous and CPU-bound. Bridge from async with
    /// `tokio::task::spawn_blocking` or equivalent.
    #[cfg(feature = "parallel")]
    pub fn ocr_all_pages(
        &self,
        render_config: &RenderConfig,
        ocr_config: &OcrConfig,
    ) -> Result<Vec<OcrPageResult>, OcrError> {
        let backend = self.require_ocr_backend()?;
        ocr_all_pages_chunked(
            self.page_count(),
            &|i| {
                self.render_page(i, render_config)
                    .map_err(|e| OcrError::RenderFailed {
                        page: i,
                        source: Arc::new(e),
                    })
            },
            backend,
            ocr_config,
            8,
        )
    }

    /// Lazy sequential iterator yielding `(page_index, OcrPageResult)`.
    ///
    /// Prefer over [`PdfDocument::ocr_all_pages`] when streaming results or short-circuiting
    /// on a condition without materialising all pag
    pub fn ocr_pages_iter<'a>(
        &'a self,
        render_config: &'a RenderConfig,
        ocr_config: &'a OcrConfig,
    ) -> impl Iterator<Item = Result<(u32, OcrPageResult), OcrError>> + use<'a> {
        (0..self.page_count())
            .map(move |i| self.ocr_page(i, render_config, ocr_config).map(|r| (i, r)))
    }

    /// OCR every page using a bounded render→infer pipeline.
    ///
    /// Renders pages on the calling thread and dispatches inference to a
    /// dedicated worker thread via a bounded channel. The channel capacity
    /// controls peak memory - at capacity `N`, at most `N` rendered
    /// `DynamicImage`s are held in memory simultaneously, regardless of
    /// document length.
    ///
    /// # When to use this vs `ocr_all_pages`
    ///
    /// | Method | Memory | Throughput | Use case |
    /// |---|---|---|---|
    /// | `ocr_pages_iter` | O(1) | Sequential | Simple streaming |
    /// | `ocr_all_pages` | O(pages) | Parallel (bounded) | Batch, memory available |
    /// | `ocr_pipeline` | O(capacity) | Pipelined | Long docs, memory constrained |
    ///
    /// # Errors
    ///
    /// Fails fast - the first render or inference error terminates the
    /// pipeline and is returned to the caller. Pages already completed
    /// are discarded.
    pub fn ocr_pipeline(
        &self,
        render_config: &RenderConfig,
        ocr_config: &OcrConfig,
        channel_capacity: usize,
    ) -> Result<Vec<OcrPageResult>, OcrError> {
        let backend = self
            .ocr_backend
            .clone()
            .ok_or(OcrError::BackendNotAttached)?;
        ocr_pipeline_with(
            self.page_count(),
            |i| {
                self.render_page(i, render_config)
                    .map_err(|e| OcrError::RenderFailed {
                        page: i,
                        source: Arc::new(e),
                    })
            },
            backend,
            ocr_config.clone(),
            channel_capacity,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };

    use image::{DynamicImage, RgbImage};
    use rstest::{fixture, rstest};

    use super::*;
    use crate::{
        RenderError,
        ocr::{OcrBackend, OcrConfig, OcrPageResult},
    };

    /// 1×1 white image
    fn stub_image() -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::new(1, 1))
    }

    fn stub_result(page_index: u32) -> OcrPageResult {
        OcrPageResult {
            page_index,
            text: format!("page {page_index}"),
        }
    }

    fn ok_render(_page_index: u32) -> Result<DynamicImage, OcrError> {
        Ok(stub_image())
    }

    fn err_render(page_index: u32) -> Result<DynamicImage, OcrError> {
        Err(OcrError::RenderFailed {
            page: page_index,
            source: Arc::new(RenderError::PageOutOfRange {
                page: page_index,
                total: 0,
            }),
        })
    }

    #[fixture]
    fn ocr_config() -> OcrConfig {
        OcrConfig::default()
    }

    // ── OkBackend ─────────────────────────────────────────────────────────────

    struct OkBackend(Arc<AtomicU32>);

    impl OkBackend {
        fn new() -> (Self, Arc<AtomicU32>) {
            let c = Arc::new(AtomicU32::new(0));
            (Self(Arc::clone(&c)), c)
        }
    }

    impl OcrBackend for OkBackend {
        fn run(
            &self,
            _image: &DynamicImage,
            page_index: u32,
            _config: &OcrConfig,
        ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(stub_result(page_index))
        }
    }

    // ── ErrBackend ────────────────────────────────────────────────────────────

    struct ErrBackend;

    impl OcrBackend for ErrBackend {
        fn run(
            &self,
            _image: &DynamicImage,
            page_index: u32,
            _config: &OcrConfig,
        ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
            Err(format!("inference failed on page {page_index}").into())
        }
    }

    // ── PanicBackend ──────────────────────────────────────────────────────────

    struct PanicBackend;

    impl OcrBackend for PanicBackend {
        fn run(
            &self,
            _image: &DynamicImage,
            _page_index: u32,
            _config: &OcrConfig,
        ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
            panic!("backend panicked");
        }
    }

    #[rstest]
    fn test_ocr_page_with_ok(ocr_config: OcrConfig) {
        let (backend, count) = OkBackend::new();
        let result = ocr_page_with(0, &ok_render, &backend, &ocr_config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().page_index, 0);
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[rstest]
    fn test_ocr_page_with_render_error(ocr_config: OcrConfig) {
        let (backend, count) = OkBackend::new();
        let result = ocr_page_with(3, &err_render, &backend, &ocr_config);
        assert!(result.is_err());
        // Render failed before inference - backend must not have been called.
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }

    #[rstest]
    fn test_ocr_page_with_inference_error(ocr_config: OcrConfig) {
        let result = ocr_page_with(0, &ok_render, &ErrBackend, &ocr_config);
        assert!(matches!(
            result.unwrap_err(),
            OcrError::InferenceFailed { page: 0, .. }
        ));
    }

    #[cfg(not(feature = "parallel"))]
    #[rstest]
    fn test_seq_returns_all_pages(ocr_config: OcrConfig) {
        let (backend, count) = OkBackend::new();
        let results = ocr_all_pages_seq(4, &ok_render, &backend, &ocr_config).unwrap();
        assert_eq!(results.len(), 4);
        assert_eq!(count.load(Ordering::Relaxed), 4);
        // Page indices must be preserved in order.
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.page_index, i as u32);
        }
    }

    #[cfg(not(feature = "parallel"))]
    #[rstest]
    fn test_seq_zero_pages(ocr_config: OcrConfig) {
        let (backend, _) = OkBackend::new();
        let results = ocr_all_pages_seq(0, &ok_render, &backend, &ocr_config).unwrap();
        assert!(results.is_empty());
    }

    #[cfg(not(feature = "parallel"))]
    #[rstest]
    fn test_seq_fails_fast_on_render_error(ocr_config: OcrConfig) {
        // Render fails on page 2; pages 0 and 1 succeed.
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let render = |i: u32| -> Result<DynamicImage, OcrError> {
            call_count_clone.fetch_add(1, Ordering::Relaxed);
            if i == 2 { err_render(i) } else { ok_render(i) }
        };

        let (backend, infer_count) = OkBackend::new();
        let result = ocr_all_pages_seq(5, &render, &backend, &ocr_config);
        assert!(result.is_err());
        // Render was called for pages 0, 1, 2 - stopped at the error.
        assert_eq!(call_count.load(Ordering::Relaxed), 3);
        // Inference ran for pages 0 and 1 only.
        assert_eq!(infer_count.load(Ordering::Relaxed), 2);
    }

    #[cfg(not(feature = "parallel"))]
    #[rstest]
    fn test_seq_fails_fast_on_inference_error(ocr_config: OcrConfig) {
        let result = ocr_all_pages_seq(3, &ok_render, &ErrBackend, &ocr_config);
        assert!(matches!(
            result.unwrap_err(),
            OcrError::InferenceFailed { page: 0, .. }
        ));
    }

    #[cfg(feature = "parallel")]
    #[rstest]
    #[case(1, 1)]
    #[case(4, 2)]
    #[case(5, 2)] // odd page count - last chunk has 1 page
    #[case(6, 3)]
    fn test_chunked_returns_all_pages(
        #[case] page_count: u32,
        #[case] chunk_size: usize,
        ocr_config: OcrConfig,
    ) {
        let (backend, count) = OkBackend::new();
        let results =
            ocr_all_pages_chunked(page_count, &ok_render, &backend, &ocr_config, chunk_size)
                .unwrap();
        assert_eq!(results.len(), page_count as usize);
        assert_eq!(count.load(Ordering::Relaxed), page_count);
    }

    #[cfg(feature = "parallel")]
    #[rstest]
    fn test_chunked_zero_pages(ocr_config: OcrConfig) {
        let (backend, _) = OkBackend::new();
        let results = ocr_all_pages_chunked(0, &ok_render, &backend, &ocr_config, 2).unwrap();
        assert!(results.is_empty());
    }

    #[cfg(feature = "parallel")]
    #[rstest]
    fn test_chunked_inference_error_propagates(ocr_config: OcrConfig) {
        let result = ocr_all_pages_chunked(3, &ok_render, &ErrBackend, &ocr_config, 2);
        assert!(matches!(
            result.unwrap_err(),
            OcrError::InferenceFailed { .. }
        ));
    }

    #[rstest]
    fn test_pipeline_returns_all_pages(ocr_config: OcrConfig) {
        let (backend, count) = OkBackend::new();
        let results = ocr_pipeline_with(4, ok_render, Arc::new(backend), ocr_config, 2).unwrap();
        assert_eq!(results.len(), 4);
        assert_eq!(count.load(Ordering::Relaxed), 4);
    }

    #[rstest]
    fn test_pipeline_zero_pages(ocr_config: OcrConfig) {
        let (backend, _) = OkBackend::new();
        let results = ocr_pipeline_with(0, ok_render, Arc::new(backend), ocr_config, 2).unwrap();
        assert!(results.is_empty());
    }

    #[rstest]
    fn test_pipeline_render_error_terminates(ocr_config: OcrConfig) {
        let (backend, infer_count) = OkBackend::new();
        // Render fails on page 1.
        let render = |i: u32| if i == 1 { err_render(i) } else { ok_render(i) };
        let result = ocr_pipeline_with(4, render, Arc::new(backend), ocr_config, 4);
        assert!(result.is_err());
        // Page 0 was already sent and processed; page 1 caused the render
        // thread to bail. The inference thread may have processed page 0.
        // We assert inference count is strictly less than total pages.
        assert!(infer_count.load(Ordering::Relaxed) < 4);
    }

    #[rstest]
    fn test_pipeline_inference_error_propagates(ocr_config: OcrConfig) {
        let result = ocr_pipeline_with(2, ok_render, Arc::new(ErrBackend), ocr_config, 2);
        assert!(matches!(
            result.unwrap_err(),
            OcrError::InferenceFailed { .. }
        ));
    }

    #[rstest]
    fn test_pipeline_backend_panic_returns_error(ocr_config: OcrConfig) {
        let result = ocr_pipeline_with(1, ok_render, Arc::new(PanicBackend), ocr_config, 1);
        assert!(matches!(
            result.unwrap_err(),
            OcrError::InferenceFailed { .. }
        ));
    }

    // ── capacity boundary ─────────────────────────────────────────────────────
    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(10)]
    fn test_pipeline_various_capacities(#[case] capacity: usize, ocr_config: OcrConfig) {
        let (backend, _) = OkBackend::new();
        let results =
            ocr_pipeline_with(5, ok_render, Arc::new(backend), ocr_config, capacity).unwrap();
        assert_eq!(results.len(), 5);
    }
}
