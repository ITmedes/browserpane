use std::fs;
use std::path::{Path, PathBuf};

const NOTICE_LINE_LIMIT: usize = 350;
const WARNING_LINE_LIMIT: usize = 500;
const STRICT_ENV: &str = "BPANE_GATEWAY_SOURCE_SIZE_STRICT";

#[derive(Debug, Clone)]
struct SourceFileSize {
    relative_path: PathBuf,
    line_count: usize,
}

#[test]
fn report_gateway_source_file_sizes() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = collect_rust_source_sizes(&source_root, &source_root);
    files.sort_by(|left, right| {
        right
            .line_count
            .cmp(&left.line_count)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });

    let notable: Vec<_> = files
        .iter()
        .filter(|file| file.line_count > NOTICE_LINE_LIMIT)
        .cloned()
        .collect();
    let oversized: Vec<_> = files
        .iter()
        .filter(|file| file.line_count > WARNING_LINE_LIMIT)
        .cloned()
        .collect();

    println!(
        "bpane-gateway source size report: {} Rust files, {} over {}, {} over {}",
        files.len(),
        notable.len(),
        NOTICE_LINE_LIMIT,
        oversized.len(),
        WARNING_LINE_LIMIT
    );
    for file in notable.iter().take(15) {
        println!(
            "  {:>4} lines  {}",
            file.line_count,
            file.relative_path.display()
        );
    }

    if !oversized.is_empty() {
        println!(
            "warning: {} Rust source files exceed {} lines",
            oversized.len(),
            WARNING_LINE_LIMIT
        );
        for file in &oversized {
            println!(
                "  {:>4} lines  {}",
                file.line_count,
                file.relative_path.display()
            );
        }
    }

    if std::env::var_os(STRICT_ENV).is_some() {
        assert!(
            oversized.is_empty(),
            "bpane-gateway has Rust source files over {} lines; unset {} for warning-only mode",
            WARNING_LINE_LIMIT,
            STRICT_ENV
        );
    }
}

fn collect_rust_source_sizes(root: &Path, current: &Path) -> Vec<SourceFileSize> {
    let mut files = Vec::new();
    let entries = fs::read_dir(current).unwrap_or_else(|error| {
        panic!(
            "failed to read source directory {}: {error}",
            current.display()
        )
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read directory entry in {}: {error}",
                current.display()
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!("failed to read file type for {}: {error}", path.display())
        });

        if file_type.is_dir() {
            files.extend(collect_rust_source_sizes(root, &path));
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        files.push(SourceFileSize {
            relative_path,
            line_count: contents.lines().count(),
        });
    }

    files
}
