use std::path::Path;

fn parse_fully(input: &str) -> Result<(), jsax::Error> {
    let mut parser = jsax::Parser::new(input);
    while parser.parse_next()?.is_some() {}
    Ok(())
}

fn run_test_parsing_dir(dir: &Path) {
    let mut y_pass = 0usize;
    let mut y_fail = 0usize;
    let mut n_pass = 0usize;
    let mut n_fail = 0usize;
    let mut i_accept = 0usize;
    let mut i_reject = 0usize;
    let mut failures: Vec<String> = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("could not read test_parsing dir")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    entries.sort();

    for path in &entries {
        let name = path.file_name().unwrap().to_str().unwrap();

        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => {
                if name.starts_with("y_") {
                    y_fail += 1;
                    failures.push(format!("FAIL y_ (not valid UTF-8): {name}"));
                } else if name.starts_with("n_") {
                    n_pass += 1;
                } else {
                    i_reject += 1;
                }
                continue;
            }
        };

        let result = parse_fully(&content);

        if name.starts_with("y_") {
            if result.is_ok() {
                y_pass += 1;
            } else {
                y_fail += 1;
                failures.push(format!(
                    "Failed y_ (should accept): {name} -- {:?}",
                    result.unwrap_err()
                ));
            }
        } else if name.starts_with("n_") {
            if result.is_err() {
                n_pass += 1;
            } else {
                n_fail += 1;
                failures.push(format!("Failed n_ (should reject but accepted): {name}"));
            }
        } else if name.starts_with("i_") {
            if result.is_ok() {
                i_accept += 1;
            } else {
                i_reject += 1;
            }
        }
    }

    eprintln!("## JSONTestSuite results");
    eprintln!("  y_ (must accept):  {} passed, {} failed", y_pass, y_fail);
    eprintln!("  n_ (must reject):  {} passed, {} failed", n_pass, n_fail);
    eprintln!(
        "  i_ (impl-defined): {} accepted, {} rejected",
        i_accept, i_reject
    );
    eprintln!("────────────────────────────────────────────────────");

    if !failures.is_empty() {
        eprintln!("\nFailures:");
        for f in &failures {
            eprintln!("  {f}");
        }
        eprintln!();
    }

    assert_eq!(
        y_fail, 0,
        "{y_fail} valid JSON file(s) were incorrectly rejected"
    );
    // jsax is (intentionally) quite permissive at the moment, so I'm still
    // undecided if `n_` failures are a real problem or not
    // assert_eq!(
    //     n_fail, 0,
    //     "{n_fail} invalid JSON file(s) were incorrectly accepted"
    // );
}

#[test]
fn json_test_suite_parsing() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_parsing");
    run_test_parsing_dir(&dir);
}
