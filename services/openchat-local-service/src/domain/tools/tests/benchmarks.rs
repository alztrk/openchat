//! Split from tests.rs (9 items).
use super::*;

#[test]
#[ignore = "manual tool-output benchmark; run with --ignored --nocapture"]
fn benchmark_tool_latency_and_output_size() {
    let directory = TestDirectory::new();
    let mut dataset_bytes = 0usize;
    for file_index in 0..500 {
        let line_count = if file_index == 0 { 600 } else { 100 };
        let mut content = String::with_capacity(line_count * 90);
        for line_index in 0..line_count {
            content.push_str(&format!(
                "// file={file_index:04} line={line_index:04} {}{}\n",
                "x".repeat(40),
                if line_index == 30 {
                    " needle target"
                } else {
                    ""
                },
            ));
        }
        dataset_bytes += content.len();
        directory.write(&format!("src/file_{file_index:04}.rs"), &content);
    }
    let hidden_content = "needle hidden\n";
    dataset_bytes += hidden_content.len();
    directory.write(".hidden/secret.txt", hidden_content);

    let root = directory.root();
    let measurements = [
        measure("list_files", || {
            list_files(root, "src", 0, 100).expect("benchmark list")
        }),
        measure("list_files_legacy_path_walker", || {
            let listing = super::super::read_directory_listing_legacy(
                &super::super::canonical_root(root)
                    .expect("canonical benchmark workspace root")
                    .join("src"),
                false,
                100,
            )
            .expect("benchmark direct list");
            super::super::directory_listing_response(Path::new("src"), &listing, 0, 100)
        }),
        measure("search_files", || {
            search_files(root, "", "needle", false, 0, 40).expect("benchmark search")
        }),
        measure("search_files_direct_no_app_cache", || {
            let canonical_root =
                super::super::canonical_root(root).expect("canonical benchmark workspace root");
            super::super::search::search_files_direct(
                &canonical_root,
                &canonical_root,
                "needle",
                false,
                false,
                0,
                40,
            )
            .expect("benchmark direct search")
        }),
        measure("search_files_no_match", || {
            search_files(root, "", "no-such-marker", false, 0, 40)
                .expect("benchmark complete search")
        }),
        measure("read_file", || {
            read_file(root, "src/file_0000.rs", 1, 200).expect("benchmark read")
        }),
        measure("read_file_start_late", || {
            read_file(root, "src/file_0000.rs", 500, 100).expect("benchmark late read")
        }),
        measure("get_file_info", || {
            get_file_info(root, "src/file_0000.rs").expect("benchmark file info")
        }),
    ];

    eprintln!(
        "dataset_files=501 dataset_bytes={dataset_bytes} benchmark_iterations=500 warmup_iterations=10"
    );
    for measurement in measurements {
        eprintln!(
            "tool={} operation_p50_ms={:.3} operation_p95_ms={:.3} operation_p99_ms={:.3} serialization_p50_us={:.3} serialization_p95_us={:.3} serialization_p99_us={:.3} output_bytes={} output_chars={} rough_tokens_chars_div_4={}",
            measurement.name,
            measurement.p50.as_secs_f64() * 1000.0,
            measurement.p95.as_secs_f64() * 1000.0,
            measurement.p99.as_secs_f64() * 1000.0,
            measurement.serialization_p50.as_secs_f64() * 1_000_000.0,
            measurement.serialization_p95.as_secs_f64() * 1_000_000.0,
            measurement.serialization_p99.as_secs_f64() * 1_000_000.0,
            measurement.output_bytes,
            measurement.output_chars,
            measurement.output_chars.div_ceil(4),
        );
    }
}

#[test]
#[ignore = "manual monorepo-scale benchmark; run with --ignored --nocapture"]
fn benchmark_search_at_monorepo_scales() {
    for file_count in [10_000, 100_000] {
        benchmark_search_scale(file_count);
    }
}

fn benchmark_search_scale(file_count: usize) {
    let directory = TestDirectory::new_on_repository_volume();
    directory.write(".gitignore", "");
    let mut dataset_bytes = 0usize;
    for file_index in 0..file_count {
        let content = format!("file={file_index:06} {}\n", "content ".repeat(16));
        dataset_bytes += content.len();
        directory.write(&format!("src/file_{file_index:06}.txt"), &content);
    }
    let canonical_source = directory
        .0
        .join("src")
        .canonicalize()
        .expect("canonical benchmark source directory");
    let source_entries = fs::read_dir(&canonical_source)
        .expect("read benchmark source directory")
        .map(|entry| entry.expect("read benchmark source entry").path())
        .collect::<Vec<_>>();
    let late_match_paths = source_entries
        .get(100..141)
        .expect("benchmark has a late search page")
        .to_vec();
    for late_match_path in &late_match_paths {
        let mut late_match_content =
            fs::read_to_string(late_match_path).expect("read benchmark late-match file");
        late_match_content.push_str("late-only-marker\n");
        dataset_bytes += "late-only-marker\n".len();
        fs::write(late_match_path, late_match_content).expect("write benchmark late marker");
    }
    let late_match_relative = Path::new("src").join(
        late_match_paths[0]
            .file_name()
            .and_then(|name| name.to_str())
            .expect("benchmark file name is valid UTF-8"),
    );

    eprintln!("scale={file_count} dataset_bytes={dataset_bytes} warmup_iterations=1");
    let listing_started = Instant::now();
    let listing_cold =
        list_files(directory.root(), "src", 0, 100).expect("benchmark cold directory listing");
    let listing_cold_elapsed = listing_started.elapsed();
    let listing = measure_iterations("list_files", 7, 1, || {
        list_files(directory.root(), "src", 0, 100).expect("benchmark large listing")
    });
    let cold_started = Instant::now();
    let cold = search_files(directory.root(), "", "no-such-marker", false, 0, 40)
        .expect("benchmark cold full search");
    let cold_elapsed = cold_started.elapsed();
    let (canonical_root, source_directory, root_handle, source_handle, explicit_filter_scope) =
        super::super::paths::resolve_directory_for_listing(directory.root(), "src")
            .expect("resolve benchmark listing directory");
    let ignore_matchers = super::super::listing::load_gitignore_matchers(
        &canonical_root,
        &source_directory,
        &root_handle,
        &source_handle,
    )
    .expect("load benchmark ignore rules");
    benchmark_listing_phases(directory.root(), &ignore_matchers);
    let direct_listing = measure_iterations("list_files_entry_scan_same_rules", 3, 1, || {
        let listing = super::super::listing::read_directory_listing_from_capability(
            &source_handle,
            &source_directory,
            &ignore_matchers,
            explicit_filter_scope,
            100,
        )
        .expect("benchmark directory entry scan");
        super::super::directory_listing_response(Path::new("src"), &listing, 0, 100)
    });
    let unfiltered_listing = measure_iterations("list_files_entry_scan_empty_rules", 3, 1, || {
        let listing = super::super::read_directory_listing(&source_directory, false, 100)
            .expect("benchmark entry scan without ignore rules");
        super::super::directory_listing_response(Path::new("src"), &listing, 0, 100)
    });
    let direct_search = measure_iterations("search_files_direct_no_app_cache", 3, 1, || {
        super::super::search::search_files_direct(
            &canonical_root,
            &canonical_root,
            "no-such-marker",
            false,
            false,
            0,
            40,
        )
        .expect("benchmark direct full search")
    });
    let late_match = measure_iterations("search_files_late_match_no_app_cache", 3, 1, || {
        search_files(directory.root(), "", "late-only-marker", false, 0, 40)
            .expect("benchmark late match search")
    });
    let late_match_output = search_files(directory.root(), "", "late-only-marker", false, 0, 40)
        .expect("verify benchmark late match");
    assert_eq!(
        late_match_output["matches"][0]["path"],
        json!(late_match_relative.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        late_match_output["matches"].as_array().map(Vec::len),
        Some(40)
    );
    assert_eq!(late_match_output["nextOffset"], json!(40));
    let repeated = measure_iterations("search_files_direct_no_app_cache_repeat", 7, 1, || {
        search_files(directory.root(), "", "another-no-marker", false, 0, 40)
            .expect("benchmark repeated direct full search")
    });
    let common = measure_iterations("search_files_common_term_no_app_cache", 7, 1, || {
        search_files(directory.root(), "", "content", false, 0, 40)
            .expect("benchmark common search")
    });
    eprintln!(
        "scale={file_count} tool=list_files_cold elapsed_ms={:.3} output_bytes={} truncated={}",
        listing_cold_elapsed.as_secs_f64() * 1000.0,
        serde_json::to_vec(&listing_cold).map_or(0, |output| output.len()),
        listing_cold["truncated"].as_bool().unwrap_or(true),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={}",
        listing.name,
        listing.p50.as_secs_f64() * 1000.0,
        listing.p95.as_secs_f64() * 1000.0,
        listing.output_bytes,
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} rough_tokens_chars_div_4={}",
        direct_listing.name,
        direct_listing.p50.as_secs_f64() * 1000.0,
        direct_listing.p95.as_secs_f64() * 1000.0,
        direct_listing.output_bytes,
        direct_listing.output_chars.div_ceil(4),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} rough_tokens_chars_div_4={}",
        unfiltered_listing.name,
        unfiltered_listing.p50.as_secs_f64() * 1000.0,
        unfiltered_listing.p95.as_secs_f64() * 1000.0,
        unfiltered_listing.output_bytes,
        unfiltered_listing.output_chars.div_ceil(4),
    );
    eprintln!(
        "scale={file_count} tool=search_files_cold elapsed_ms={:.3} output_bytes={} truncated={}",
        cold_elapsed.as_secs_f64() * 1000.0,
        serde_json::to_vec(&cold).map_or(0, |output| output.len()),
        cold["truncated"].as_bool().unwrap_or(true),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={}",
        repeated.name,
        repeated.p50.as_secs_f64() * 1000.0,
        repeated.p95.as_secs_f64() * 1000.0,
        repeated.output_bytes,
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} rough_tokens_chars_div_4={}",
        direct_search.name,
        direct_search.p50.as_secs_f64() * 1000.0,
        direct_search.p95.as_secs_f64() * 1000.0,
        direct_search.output_bytes,
        direct_search.output_chars.div_ceil(4),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} rough_tokens_chars_div_4={}",
        late_match.name,
        late_match.p50.as_secs_f64() * 1000.0,
        late_match.p95.as_secs_f64() * 1000.0,
        late_match.output_bytes,
        late_match.output_chars.div_ceil(4),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={}",
        common.name,
        common.p50.as_secs_f64() * 1000.0,
        common.p95.as_secs_f64() * 1000.0,
        common.output_bytes,
    );
}

fn benchmark_listing_phases(root: &str, ignore_matchers: &[ignore::gitignore::Gitignore]) {
    let (root_path, source_path, root_handle, source_handle, explicit_filter_scope) =
        super::super::paths::resolve_directory_for_listing(root, "src")
            .expect("resolve benchmark listing phases");

    measure_duration("list_resolve_and_handle_validation", 7, 1, || {
        let _ = super::super::paths::resolve_directory_for_listing(root, "src")
            .expect("benchmark resolve listing directory");
    });
    measure_duration("list_load_gitignore_matchers", 7, 1, || {
        let _ = super::super::listing::load_gitignore_matchers(
            &root_path,
            &source_path,
            &root_handle,
            &source_handle,
        )
        .expect("benchmark load ignore rules");
    });
    measure_duration("list_enumerate_filter_and_sort", 7, 1, || {
        let _ = super::super::listing::read_directory_listing_from_capability(
            &source_handle,
            &source_path,
            ignore_matchers,
            explicit_filter_scope,
            100,
        )
        .expect("benchmark enumerate directory entries");
    });
    measure_duration("list_post_scan_handle_validation", 7, 1, || {
        super::super::paths::verify_directory_handles(
            &root_path,
            &source_path,
            &root_handle,
            &source_handle,
        )
        .expect("benchmark validate listing handles");
    });
}

struct Measurement {
    name: &'static str,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    serialization_p50: Duration,
    serialization_p95: Duration,
    serialization_p99: Duration,
    output_bytes: usize,
    output_chars: usize,
}

fn measure(name: &'static str, operation: impl FnMut() -> serde_json::Value) -> Measurement {
    measure_iterations(name, 500, 10, operation)
}

fn measure_iterations(
    name: &'static str,
    iterations: usize,
    warmup_iterations: usize,
    mut operation: impl FnMut() -> serde_json::Value,
) -> Measurement {
    for _ in 0..warmup_iterations {
        let result = operation();
        let _ = serde_json::to_vec(&result).expect("serialize benchmark warmup output");
    }

    let mut durations = Vec::with_capacity(iterations);
    let mut serialization_durations = Vec::with_capacity(iterations);
    let mut output = Vec::new();
    for _ in 0..iterations {
        let started = Instant::now();
        let result = operation();
        durations.push(started.elapsed());

        let serialization_started = Instant::now();
        output = serde_json::to_vec(&result).expect("serialize benchmark output");
        serialization_durations.push(serialization_started.elapsed());
    }
    durations.sort_unstable();
    serialization_durations.sort_unstable();
    let output_text = std::str::from_utf8(&output).expect("tool output is valid UTF-8");
    Measurement {
        name,
        p50: percentile(&durations, 50),
        p95: percentile(&durations, 95),
        p99: percentile(&durations, 99),
        serialization_p50: percentile(&serialization_durations, 50),
        serialization_p95: percentile(&serialization_durations, 95),
        serialization_p99: percentile(&serialization_durations, 99),
        output_bytes: output.len(),
        output_chars: output_text.chars().count(),
    }
}

fn percentile(durations: &[Duration], percentile: usize) -> Duration {
    durations[(durations.len() - 1) * percentile / 100]
}

fn measure_duration(
    name: &str,
    iterations: usize,
    warmup_iterations: usize,
    mut operation: impl FnMut(),
) {
    for _ in 0..warmup_iterations {
        operation();
    }
    let mut durations = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        operation();
        durations.push(started.elapsed());
    }
    durations.sort_unstable();
    eprintln!(
        "benchmark_phase={name} p50_ms={:.3} p95_ms={:.3}",
        percentile(&durations, 50).as_secs_f64() * 1000.0,
        percentile(&durations, 95).as_secs_f64() * 1000.0,
    );
}

#[test]
#[ignore = "manual filesystem latency benchmark; run with --ignored --nocapture"]
fn benchmark_write_and_edit_performance_simulation() {
    use std::time::Instant;

    let directory = TestDirectory::new();
    let root = directory.root();

    // Test with a 100 KB payload
    let chunk = "The quick brown fox jumps over the lazy dog. Rust memory safety and zero-cost abstractions.\n";
    let iterations = 100_000 / chunk.len();
    let mut large_text = String::with_capacity(100_000);
    for _ in 0..iterations {
        large_text.push_str(chunk);
    }
    large_text.push_str("UNIQUE_NEEDLE_FOR_BENCHMARK");

    // Measure write latency
    let write_start = Instant::now();
    let write_res = super::super::write_file(root, "bench_100kb.txt", &large_text);
    let write_duration = write_start.elapsed();
    assert!(write_res.is_ok());
    println!("[BENCHMARK] write_file 100KB: {:?}", write_duration);

    // Warm-up / let OS file creation filter locks settle
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Measure edit latency on settled 100KB file
    let edit_start = Instant::now();
    let edit_res = super::super::edit_file(
        root,
        "bench_100kb.txt",
        "UNIQUE_NEEDLE_FOR_BENCHMARK",
        "REPLACED_SUCCESSFULLY",
    );
    let edit_duration = edit_start.elapsed();
    assert!(edit_res.is_ok());
    println!(
        "[BENCHMARK] edit_file in 100KB (single-handle): {:?}",
        edit_duration
    );

    // Verify both are well under threshold
    assert!(
        write_duration.as_millis() < 50,
        "write_file took too long: {:?}",
        write_duration
    );
    assert!(
        edit_duration.as_millis() < 50,
        "edit_file took too long: {:?}",
        edit_duration
    );
}

#[tokio::test]
#[ignore = "manual live web smoke test requires public network access"]
async fn live_manual_benchmark_web_search_and_read_url() {
    use super::super::web_search::{execute_read_url, execute_web_search};
    use std::time::Instant;

    println!("\n========== [CANLI TEST BASLANGICI] ==========");

    // 1. Web Search Testi (Limit: 10)
    let query = "Rust tokio tutorial";
    let start_search = Instant::now();
    let search_res = execute_web_search(query, Some(10)).await;
    let search_elapsed = start_search.elapsed();

    println!("1. WEB SEARCH SONUCU (Limit: 10):");
    println!("Sorgu: \"{}\"", query);
    println!("Gecikme: {} ms", search_elapsed.as_millis());

    let mut first_url = None;
    match search_res {
        Ok(val) => {
            println!("Durum: Basarili");
            let total = val["total_results"].as_i64().unwrap_or(0);
            println!("Toplam sonuc sayisi: {}", total);
            if let Some(results) = val["results"].as_array() {
                for (i, item) in results.iter().enumerate() {
                    let title = item["title"].as_str().unwrap_or("");
                    let url = item["url"].as_str().unwrap_or("");
                    let engine = item["engine"].as_str().unwrap_or("");
                    let snippet = item["snippet"].as_str().unwrap_or("");
                    println!("  [{}] Motor: {} | Baslik: {}", i + 1, engine, title);
                    println!("      URL: {}", url);
                    println!("      Ozet: {}", snippet);
                    if first_url.is_none() && !url.is_empty() {
                        first_url = Some(url.to_owned());
                    }
                }
            }
        }
        Err(err) => {
            println!("Hata: {}", err);
        }
    }

    println!("\n--------------------------------------------");

    // 2. Read URL Content Testi
    let target_url = first_url.unwrap_or_else(|| "https://example.com".to_owned());
    let start_read = Instant::now();
    let read_res = execute_read_url(&target_url, Some(3000)).await;
    let read_elapsed = start_read.elapsed();

    println!("2. READ URL CONTENT SONUCU:");
    println!("Hedef URL: {}", target_url);
    println!("Gecikme: {} ms", read_elapsed.as_millis());

    match read_res {
        Ok(val) => {
            println!("Durum: Basarili");
            println!("Baslik: {}", val["title"].as_str().unwrap_or(""));
            println!("Karakter Uzunlugu: {}", val["length"]);
            println!("Kesildi mi (truncated): {}", val["truncated"]);
            let content = val["content"].as_str().unwrap_or("");
            let preview: String = content.chars().take(500).collect();
            println!(
                "\nIcerik Onizleme (Ilk 500 karakter):\n---\n{}\n---",
                preview
            );
        }
        Err(err) => {
            println!("Hata: {}", err);
        }
    }

    // 3. Turkce Arama ve Sayfa Okuma Testi (Limit: 10)
    println!("\n--------------------------------------------");
    let tr_query = "Turkiye yapay zeka ekosistemi";
    let start_tr = Instant::now();
    let tr_res = execute_web_search(tr_query, Some(10)).await;
    let tr_elapsed = start_tr.elapsed();

    println!("3. TURKCE ARAMA SONUCU (Limit: 10):");
    println!("Sorgu: \"{}\"", tr_query);
    println!("Gecikme: {} ms", tr_elapsed.as_millis());
    let mut tr_url = None;
    if let Ok(val) = tr_res
        && let Some(results) = val["results"].as_array()
    {
        for (i, item) in results.iter().enumerate() {
            let title = item["title"].as_str().unwrap_or("");
            let url = item["url"].as_str().unwrap_or("");
            let engine = item["engine"].as_str().unwrap_or("");
            println!("  [{}] Motor: {} | Baslik: {}", i + 1, engine, title);
            println!("      URL: {}", url);
            if tr_url.is_none() && !url.is_empty() {
                tr_url = Some(url.to_owned());
            }
        }
    }

    if let Some(url) = tr_url {
        let start_tr_read = Instant::now();
        let tr_read_res = execute_read_url(&url, Some(1500)).await;
        let tr_read_elapsed = start_tr_read.elapsed();
        println!(
            "\nTURKCE SAYFA OKUMA Gecikmesi: {} ms",
            tr_read_elapsed.as_millis()
        );
        if let Ok(val) = tr_read_res {
            println!("Sayfa Basligi: {}", val["title"].as_str().unwrap_or(""));
            println!("Okunan Karakter: {}", val["length"]);
        }
    }

    // 4. Cache Testi
    println!("\n--------------------------------------------");
    let start_cache = Instant::now();
    let cache_res = execute_web_search(query, Some(5)).await;
    let cache_elapsed = start_cache.elapsed();
    println!("4. ONBELLEK (CACHE) HIZ TESTI:");
    println!("Ayni sorgu tekrar calistirildi.");
    println!(
        "Onbellek Gecikmesi: {} mikro-saniye ({} ms)",
        cache_elapsed.as_micros(),
        cache_elapsed.as_millis()
    );
    assert!(cache_res.is_ok());

    println!("========== [CANLI TEST BITTI] ==========\n");
}

#[tokio::test]
#[ignore = "manual GitHub web smoke test requires public network access"]
async fn live_manual_github_search() {
    use super::super::web_search::execute_web_search;

    let query = "site:github.com/rust-lang/rust";
    let search = execute_web_search(query, Some(5))
        .await
        .expect("execute a GitHub repository search");
    let results = search["results"]
        .as_array()
        .expect("web search returns a results array");
    let urls = results
        .iter()
        .filter_map(|result| result["url"].as_str())
        .collect::<Vec<_>>();
    println!("Query `{query}` returned: {urls:?}");

    assert!(
        urls.iter().any(|url| {
            url::Url::parse(url).is_ok_and(|url| {
                url.host_str() == Some("github.com")
                    && (url.path() == "/rust-lang/rust"
                        || url.path().starts_with("/rust-lang/rust/"))
            })
        }),
        "site-restricted search should return a page in the requested GitHub repository"
    );
}

#[tokio::test]
#[ignore = "manual GitHub page-read smoke test requires public network access"]
async fn live_manual_github_read_url() {
    use super::super::web_search::execute_read_url;

    let page = execute_read_url("https://github.com/rust-lang/rust", Some(2_000))
        .await
        .expect("read the public GitHub repository page");
    assert_eq!(page["url"], "https://github.com/rust-lang/rust");
    assert!(
        page["title"]
            .as_str()
            .is_some_and(|title| title.to_ascii_lowercase().contains("rust")),
        "GitHub page title should identify the Rust repository"
    );
    assert!(
        page["content"]
            .as_str()
            .is_some_and(|content| !content.trim().is_empty()),
        "GitHub page should yield readable content"
    );
}
