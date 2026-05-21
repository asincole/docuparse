//! Generates synthetic PDF fixtures for examples and integration tests.
//! Run: cargo run --example generate_fixtures

use std::path::Path;

use fake::{
    Fake,
    faker::{
        address::en::*, company::en::*, internet::en::*, job::en::*, lorem::en::*, name::en::*,
        phone_number::en::*,
    },
};
use lopdf::{
    Document, Object, Stream,
    content::{Content, Operation},
    dictionary,
};

fn main() {
    let out = Path::new("examples/fixtures");
    std::fs::create_dir_all(out).expect("failed to create fixtures dir");

    let person = Person::generate();
    println!("generating fixtures for: {}", person.full_name);

    create_native_pdf(&out.join("sample_native.pdf"), &person);
    println!("  sample_native.pdf  - 4 pages, dense text");

    create_scanned_pdf(&out.join("sample_scanned.pdf"));
    println!("  sample_scanned.pdf - 2 pages, images only");

    create_mixed_pdf(&out.join("sample_mixed.pdf"), &person);
    println!("  sample_mixed.pdf   - 6 pages, mixed text + scanned");

    create_large_pdf(&out.join("sample_large.pdf"), &person);
    println!("  sample_large.pdf   - 50 pages, dense varied text");

    println!("\ndone - examples/fixtures/");
}

// ── Person ────────────────────────────────────────────────────────────────────

struct Person {
    full_name: String,
    first_name: String,
    email: String,
    phone: String,
    city: String,
    country: String,
    street: String,
    postcode: String,
    job_title: String,
    company_a: String,
    company_b: String,
    company_c: String,
    university: String,
    degree: String,
    summary: String,
    paragraphs: Vec<String>,
    bullet_sets: Vec<Vec<String>>,
    skills: Vec<String>,
    lang_a: String,
    lang_b: String,
}

impl Person {
    fn generate() -> Self {
        let first: String = FirstName().fake();
        let last: String = LastName().fake();

        let tech_skills = [
            "Rust",
            "Go",
            "Python",
            "TypeScript",
            "Kubernetes",
            "PostgreSQL",
            "Redis",
            "Kafka",
            "AWS",
            "GCP",
            "Docker",
            "gRPC",
            "REST",
            "GraphQL",
            "Terraform",
            "Linux",
            "Prometheus",
            "OpenTelemetry",
        ];
        let skill_count = (6..12usize).fake::<usize>();
        let skills: Vec<String> = tech_skills[..skill_count]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let degrees = [
            "BSc Computer Science",
            "MSc Software Engineering",
            "BSc Mathematics",
            "MEng Electrical Engineering",
            "BSc Information Systems",
            "MSc Distributed Systems",
        ];
        let universities = [
            "Technical University of Berlin",
            "University of Vienna",
            "ETH Zurich",
            "KU Leuven",
            "University of Warsaw",
            "Delft University of Technology",
        ];
        let languages = [
            "English",
            "German",
            "French",
            "Spanish",
            "Polish",
            "Dutch",
            "Italian",
            "Portuguese",
        ];

        let degree = degrees[(0..degrees.len()).fake::<usize>()].to_string();
        let university = universities[(0..universities.len()).fake::<usize>()].to_string();
        let lang_a = languages[(0..languages.len()).fake::<usize>()].to_string();
        let lang_b = languages[(0..languages.len()).fake::<usize>()].to_string();

        // Generate multiple paragraphs of realistic body text
        let paragraphs: Vec<String> = (0..6)
            .map(|_| {
                let sentences: Vec<String> = Sentences(3..6).fake();
                sentences.join(" ")
            })
            .collect();

        // Generate bullet point sets for experience sections
        let bullet_sets: Vec<Vec<String>> = (0..4)
            .map(|_| {
                let count = (3..6usize).fake::<usize>();
                (0..count)
                    .map(|_| {
                        let s: String = Sentence(8..15).fake();
                        format!("- {s}")
                    })
                    .collect()
            })
            .collect();

        let summary_sentences: Vec<String> = Sentences(3..5).fake();
        let summary = summary_sentences.join(" ");

        Self {
            full_name: format!("{first} {last}"),
            first_name: first,
            email: SafeEmail().fake(),
            phone: PhoneNumber().fake(),
            city: CityName().fake(),
            country: CountryName().fake(),
            street: StreetName().fake(),
            postcode: PostCode().fake(),
            job_title: format!("{} Engineer", Seniority().fake::<String>()),
            company_a: CompanyName().fake(),
            company_b: CompanyName().fake(),
            company_c: CompanyName().fake(),
            university,
            degree,
            summary,
            paragraphs,
            bullet_sets,
            skills,
            lang_a,
            lang_b,
        }
    }

    fn address_line(&self) -> String {
        format!(
            "{}, {} {}, {}",
            self.street, self.postcode, self.city, self.country
        )
    }
}

// ── PDF primitives ────────────────────────────────────────────────────────────

fn add_font(doc: &mut Document) -> lopdf::ObjectId {
    doc.add_object(dictionary! {
        "Type"     => "Font",
        "Subtype"  => "Type1",
        "BaseFont" => "Helvetica",
    })
}

fn add_bold_font(doc: &mut Document) -> lopdf::ObjectId {
    doc.add_object(dictionary! {
        "Type"     => "Font",
        "Subtype"  => "Type1",
        "BaseFont" => "Helvetica-Bold",
    })
}

fn add_resources_two_fonts(
    doc: &mut Document,
    font_id: lopdf::ObjectId,
    bold_id: lopdf::ObjectId,
) -> lopdf::ObjectId {
    doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
            "F2" => bold_id,
        },
    })
}

/// Wraps `text` into lines of at most `max_chars` characters, breaking on
/// word boundaries. Helvetica at 10pt: ~95 chars/line at 495pt width.
fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// A line in a rich page - carries font, size, and content.
enum Line<'a> {
    /// Bold heading
    Heading(&'a str),
    /// Normal body text (will be word-wrapped)
    Body(&'a str),
    /// Pre-formatted line, no wrapping (bullets, labels)
    Fixed(&'a str),
    /// Vertical gap
    Gap,
    /// Horizontal rule rendered as dashes
    Rule,
}

/// Builds a page from a sequence of `Line` entries.
/// Automatically paginates - returns one page object id per page needed.
fn build_rich_pages(
    doc: &mut Document,
    pages_id: lopdf::ObjectId,
    resources_id: lopdf::ObjectId,
    content: &[Line<'_>],
) -> Vec<lopdf::ObjectId> {
    const MARGIN_X: i64 = 50;
    const TOP_Y: i64 = 800;
    const BOTTOM_Y: i64 = 50;
    const BODY_SIZE: i64 = 10;
    const HEAD_SIZE: i64 = 13;
    const BODY_LEAD: i64 = 14;
    const HEAD_LEAD: i64 = 20;
    const GAP_LEAD: i64 = 8;
    const WRAP_CHARS: usize = 95;

    let mut pages = Vec::new();
    let mut current_ops: Vec<Operation> = Vec::new();
    let mut y = TOP_Y;

    macro_rules! flush {
        () => {
            current_ops.push(Operation::new("ET", vec![]));
            let ops_to_flush: Vec<Operation> = current_ops.drain(..).collect();
            let content_id = doc.add_object(Stream::new(
                dictionary! {},
                Content {
                    operations: ops_to_flush,
                }
                .encode()
                .unwrap(),
            ));
            pages.push(doc.add_object(dictionary! {
                "Type"      => "Page",
                "Parent"    => pages_id,
                "MediaBox"  => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Contents"  => content_id,
                "Resources" => resources_id,
            }));
            y = TOP_Y;
            current_ops.push(Operation::new("BT", vec![]));
            current_ops.push(Operation::new("Tf", vec!["F1".into(), BODY_SIZE.into()]));
        };
    }

    current_ops.push(Operation::new("BT", vec![]));
    current_ops.push(Operation::new("Tf", vec!["F1".into(), BODY_SIZE.into()]));

    for line in content {
        match line {
            Line::Heading(text) => {
                if y < BOTTOM_Y + HEAD_LEAD * 2 {
                    flush!();
                }
                current_ops.push(Operation::new("Tf", vec!["F2".into(), HEAD_SIZE.into()]));
                current_ops.push(Operation::new(
                    "Tm",
                    vec![
                        1.into(),
                        0.into(),
                        0.into(),
                        1.into(),
                        MARGIN_X.into(),
                        y.into(),
                    ],
                ));
                current_ops.push(Operation::new("Tj", vec![Object::string_literal(*text)]));
                current_ops.push(Operation::new("Tf", vec!["F1".into(), BODY_SIZE.into()]));
                y -= HEAD_LEAD;
            }
            Line::Body(text) => {
                for wrapped in wrap(text, WRAP_CHARS) {
                    if y < BOTTOM_Y {
                        flush!();
                    }
                    current_ops.push(Operation::new(
                        "Tm",
                        vec![
                            1.into(),
                            0.into(),
                            0.into(),
                            1.into(),
                            MARGIN_X.into(),
                            y.into(),
                        ],
                    ));
                    current_ops.push(Operation::new("Tj", vec![Object::string_literal(wrapped)]));
                    y -= BODY_LEAD;
                }
            }
            Line::Fixed(text) => {
                if y < BOTTOM_Y {
                    flush!();
                }
                current_ops.push(Operation::new(
                    "Tm",
                    vec![
                        1.into(),
                        0.into(),
                        0.into(),
                        1.into(),
                        MARGIN_X.into(),
                        y.into(),
                    ],
                ));
                current_ops.push(Operation::new("Tj", vec![Object::string_literal(*text)]));
                y -= BODY_LEAD;
            }
            Line::Gap => {
                y -= GAP_LEAD;
            }
            Line::Rule => {
                current_ops.push(Operation::new(
                    "Tm",
                    vec![
                        1.into(),
                        0.into(),
                        0.into(),
                        1.into(),
                        MARGIN_X.into(),
                        y.into(),
                    ],
                ));
                current_ops.push(Operation::new(
                    "Tj",
                    vec![Object::string_literal(
                        "------------------------------------------------------------------------",
                    )],
                ));
                y -= BODY_LEAD;
            }
        }
    }

    // Flush final page
    current_ops.push(Operation::new("ET", vec![]));
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        Content {
            operations: current_ops,
        }
        .encode()
        .unwrap(),
    ));
    pages.push(doc.add_object(dictionary! {
        "Type"      => "Page",
        "Parent"    => pages_id,
        "MediaBox"  => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents"  => content_id,
        "Resources" => resources_id,
    }));

    pages
}

fn build_image_page(
    doc: &mut Document,
    pages_id: lopdf::ObjectId,
    caption: &str,
    colour: [u8; 3],
) -> lopdf::ObjectId {
    let (w, h) = (500u32, 650u32);
    let pixels: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let noise = (i % 17) as u8;
            [
                colour[0].saturating_add(noise),
                colour[1].saturating_add(noise / 2),
                colour[2].saturating_sub(noise / 3),
            ]
        })
        .collect();
    let img = image::RgbImage::from_raw(w, h, pixels).unwrap();

    let mut jpeg: Vec<u8> = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut jpeg),
        image::ImageFormat::Jpeg,
    )
    .unwrap();

    let image_id = doc.add_object(Stream::new(
        dictionary! {
            "Type"             => "XObject",
            "Subtype"          => "Image",
            "Width"            => w,
            "Height"           => h,
            "ColorSpace"       => "DeviceRGB",
            "BitsPerComponent" => 8_i64,
            "Filter"           => "DCTDecode",
        },
        jpeg,
    ));

    let font_id = add_font(doc);
    let resources_id = doc.add_object(dictionary! {
        "Font"    => dictionary! { "F1" => font_id },
        "XObject" => dictionary! { "Im1" => image_id },
    });

    let ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                495.into(),
                0.into(),
                0.into(),
                645.into(),
                50.into(),
                100.into(),
            ],
        ),
        Operation::new("Do", vec![Object::Name(b"Im1".to_vec())]),
        Operation::new("Q", vec![]),
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec!["F1".into(), 9.into()]),
        Operation::new("Td", vec![50.into(), 80.into()]),
        Operation::new("Tj", vec![Object::string_literal(caption)]),
        Operation::new("ET", vec![]),
    ];

    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        Content { operations: ops }.encode().unwrap(),
    ));

    doc.add_object(dictionary! {
        "Type"      => "Page",
        "Parent"    => pages_id,
        "MediaBox"  => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents"  => content_id,
        "Resources" => resources_id,
    })
}

fn finalise(
    doc: &mut Document,
    pages_id: lopdf::ObjectId,
    page_ids: Vec<lopdf::ObjectId>,
    path: &Path,
) {
    let count = page_ids.len() as i64;
    let kids: Vec<Object> = page_ids.into_iter().map(Into::into).collect();

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type"  => "Pages",
            "Kids"  => kids,
            "Count" => count,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type"  => "Catalog",
        "Pages" => pages_id,
    });

    doc.trailer.set("Root", catalog_id);
    doc.compress();
    doc.save(path)
        .unwrap_or_else(|e| panic!("save failed for {path:?}: {e}"));
}

// ── PDF documents ─────────────────────────────────────────────────────────────

fn create_native_pdf(path: &Path, p: &Person) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = add_font(&mut doc);
    let bold_id = add_bold_font(&mut doc);
    let res_id = add_resources_two_fonts(&mut doc, font_id, bold_id);

    let contact = format!("{} | {} | {}", p.email, p.phone, p.address_line());
    let exp_a_title = format!("{} - {} (2021-present)", p.job_title, p.company_a);
    let exp_b_title = format!("Engineer - {} (2018-2021)", p.company_b);
    let exp_c_title = format!("Junior Engineer - {} (2015-2018)", p.company_c);
    let edu_line = format!("{}, {}, 2015", p.degree, p.university);
    let lang_line = format!("{} (native), {} (professional)", p.lang_a, p.lang_b);
    let skills_line = p.skills.join(", ");

    let mut content: Vec<Line> = vec![
        Line::Heading(&p.full_name),
        Line::Fixed(&p.job_title),
        Line::Fixed(&contact),
        Line::Gap,
        Line::Heading("Professional Summary"),
        Line::Rule,
        Line::Body(&p.summary),
        Line::Body(&p.paragraphs[0]),
        Line::Gap,
        Line::Heading("Experience"),
        Line::Rule,
        Line::Fixed(&exp_a_title),
        Line::Gap,
    ];

    for bullet in &p.bullet_sets[0] {
        content.push(Line::Fixed(bullet));
    }

    content.extend([
        Line::Gap,
        Line::Body(&p.paragraphs[1]),
        Line::Gap,
        Line::Fixed(&exp_b_title),
        Line::Gap,
    ]);

    for bullet in &p.bullet_sets[1] {
        content.push(Line::Fixed(bullet));
    }

    content.extend([
        Line::Gap,
        Line::Body(&p.paragraphs[2]),
        Line::Gap,
        Line::Fixed(&exp_c_title),
        Line::Gap,
    ]);

    for bullet in &p.bullet_sets[2] {
        content.push(Line::Fixed(bullet));
    }

    content.extend([
        Line::Gap,
        Line::Heading("Education"),
        Line::Rule,
        Line::Fixed(&edu_line),
        Line::Body(&p.paragraphs[3]),
        Line::Gap,
        Line::Heading("Skills"),
        Line::Rule,
        Line::Body(&skills_line),
        Line::Gap,
        Line::Heading("Languages"),
        Line::Rule,
        Line::Fixed(&lang_line),
    ]);

    let page_ids = build_rich_pages(&mut doc, pages_id, res_id, &content);
    let n = page_ids.len();
    finalise(&mut doc, pages_id, page_ids, path);
    println!("    ({n} pages generated)");
}

fn create_scanned_pdf(path: &Path) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let page1 = build_image_page(
        &mut doc,
        pages_id,
        "Scanned document - page 1 of 2",
        [230, 230, 245],
    );
    let page2 = build_image_page(
        &mut doc,
        pages_id,
        "Scanned document - page 2 of 2",
        [245, 230, 230],
    );

    finalise(&mut doc, pages_id, vec![page1, page2], path);
}

fn create_mixed_pdf(path: &Path, p: &Person) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = add_font(&mut doc);
    let bold_id = add_bold_font(&mut doc);
    let res_id = add_resources_two_fonts(&mut doc, font_id, bold_id);

    let contact = format!("{} | {} | {}", p.email, p.phone, p.city);
    let exp_title = format!("{} - {} (2020-present)", p.job_title, p.company_a);

    let skills = &p.skills.join(", ");

    let header_content: Vec<Line> = vec![
        Line::Heading(&p.full_name),
        Line::Fixed(&p.job_title),
        Line::Fixed(&contact),
        Line::Gap,
        Line::Heading("Profile"),
        Line::Rule,
        Line::Body(&p.summary),
        Line::Body(&p.paragraphs[0]),
        Line::Gap,
        Line::Heading("Core Competencies"),
        Line::Rule,
        Line::Body(skills),
        Line::Gap,
        Line::Body(&p.paragraphs[1]),
    ];

    let text_page_ids = build_rich_pages(&mut doc, pages_id, res_id, &header_content);

    let cert_page = build_image_page(
        &mut doc,
        pages_id,
        &format!("Professional certification - {}", p.first_name),
        [245, 245, 220],
    );

    let mut exp_content: Vec<Line> = vec![
        Line::Heading("Experience"),
        Line::Rule,
        Line::Fixed(&exp_title),
        Line::Gap,
    ];
    for bullet in &p.bullet_sets[0] {
        exp_content.push(Line::Fixed(bullet));
    }
    exp_content.extend([Line::Gap, Line::Body(&p.paragraphs[2])]);

    let exp_page_ids = build_rich_pages(&mut doc, pages_id, res_id, &exp_content);

    let ref_page = build_image_page(
        &mut doc,
        pages_id,
        &format!("Reference letter for {}", p.full_name),
        [235, 250, 235],
    );

    let mut all_pages = text_page_ids;
    all_pages.push(cert_page);
    all_pages.extend(exp_page_ids);
    all_pages.push(ref_page);

    let n = all_pages.len();
    finalise(&mut doc, pages_id, all_pages, path);
    println!("    ({n} pages generated)");
}

fn create_large_pdf(path: &Path, p: &Person) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = add_font(&mut doc);
    let bold_id = add_bold_font(&mut doc);
    let res_id = add_resources_two_fonts(&mut doc, font_id, bold_id);

    // Generate varied page types to simulate a realistic long document.
    // Cycles through: report section, data table, analysis, appendix.
    let section_titles = [
        "Executive Summary",
        "Market Analysis",
        "Technical Architecture",
        "Implementation Plan",
        "Risk Assessment",
        "Financial Projections",
        "Operational Requirements",
        "Compliance and Governance",
        "Performance Metrics",
        "Recommendations",
    ];

    let mut all_page_ids: Vec<lopdf::ObjectId> = Vec::new();

    for (section_idx, _) in section_titles.iter().enumerate() {
        let title = section_titles[section_idx];

        // Generate fresh paragraphs per section for variety
        let paras: Vec<String> = (0..4)
            .map(|_| {
                let sentences: Vec<String> = Sentences(4..7).fake();
                sentences.join(" ")
            })
            .collect();

        let bullets: Vec<String> = (0..5)
            .map(|_| {
                let s: String = Sentence(8..14).fake();
                format!("- {s}")
            })
            .collect();

        let subsection_a = format!("{title} - Overview");
        let subsection_b = format!("{title} - Detail");
        let subsection_c = format!("{title} - Conclusions");
        let author_line = format!("Prepared by: {} | {}", p.full_name, p.email);
        let page_label = format!("Section {} of 10", section_idx + 1);

        let mut content: Vec<Line> = vec![
            Line::Heading(title),
            Line::Fixed(&author_line),
            Line::Fixed(&page_label),
            Line::Rule,
            Line::Gap,
            Line::Heading(&subsection_a),
            Line::Body(&paras[0]),
            Line::Body(&paras[1]),
            Line::Gap,
        ];

        for bullet in &bullets {
            content.push(Line::Fixed(bullet));
        }

        content.extend([
            Line::Gap,
            Line::Heading(&subsection_b),
            Line::Body(&paras[2]),
            Line::Gap,
        ]);

        let table_rows: Vec<String> = ["Revenue", "Cost", "Margin", "Headcount", "Incidents"]
            .iter()
            .map(|metric| {
                format!(
                    "{:<24}{:<10}{:<10}{:<10}{:<10}",
                    metric,
                    (1000..9999u32).fake::<u32>(),
                    (1000..9999u32).fake::<u32>(),
                    (1000..9999u32).fake::<u32>(),
                    (1000..9999u32).fake::<u32>(),
                )
            })
            .collect();

        // Add a simulated data table as fixed lines
        content.push(Line::Fixed(
            "Metric                  Q1        Q2        Q3        Q4",
        ));
        content.push(Line::Rule);
        for row in &table_rows {
            content.push(Line::Fixed(row));
        }
        // for metric in &["Revenue", "Cost", "Margin", "Headcount", "Incidents"] {
        //     let row = format!(
        //         "{:<24}{:<10}{:<10}{:<10}{:<10}",
        //         metric,
        //         (1000..9999u32).fake::<u32>(),
        //         (1000..9999u32).fake::<u32>(),
        //         (1000..9999u32).fake::<u32>(),
        //         (1000..9999u32).fake::<u32>(),
        //     );
        //     content.push(Line::Fixed(Box::leak(row.into_boxed_str())));
        // }

        let skills_referenced = &format!("Skills referenced: {}", p.skills.join(", "));
        content.extend([
            Line::Gap,
            Line::Heading(&subsection_c),
            Line::Body(&paras[3]),
            Line::Gap,
            Line::Fixed(skills_referenced),
        ]);

        let section_pages = build_rich_pages(&mut doc, pages_id, res_id, &content);
        all_page_ids.extend(section_pages);
    }

    let n = all_page_ids.len();
    finalise(&mut doc, pages_id, all_page_ids, path);
    println!("    ({n} pages generated)");
}
