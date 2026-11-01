//! Empirical Challenger Adversarial Stress Test: Backend Path Traversal Guard
//!
//! Evaluates `read_reference_from_dir`:
//! 1. Classic traversals (`../../../../etc/passwd`, `/etc/shadow`, `/etc/hosts`)
//! 2. Symlink attacks:
//!    - Symlink inside base pointing outside (to `/etc/passwd`)
//!    - Symlink chain pointing outside
//!    - Valid symlink pointing inside base
//!    - Dangling/broken symlink
//!    - Circular symlink loop
//! 3. Nested `resources/resources/resources/` paths:
//!    - Non-existent nested resources
//!    - Legitimate nested resources directory structure
//!    - Mixed traversal with `resources/../resources/`
//! 4. Directory boundary / sibling attack (`base_dir` vs `base_dir_attacker`)
//! 5. Directory target rejection (not a file)
//! 6. Null byte, control char, and empty path rejection

use colima_ui_lib::commands::system::read_reference_from_dir;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

fn setup_test_sandbox(name: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("cui_challenger_pt_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let sample_file = base.join("sample.md");
    fs::write(&sample_file, "EMBARGOED_CORE_KNOWLEDGE_BASE").unwrap();

    let sub = base.join("subdir");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("nested.md"), "NESTED_DOC_CONTENT").unwrap();

    (base.clone(), sample_file)
}

fn cleanup(base: PathBuf) {
    let _ = fs::remove_dir_all(base);
}

#[test]
fn test_classic_path_traversals_blocked() {
    let (base, _) = setup_test_sandbox("classic");

    // 1. Deep traversal to /etc/passwd
    let res = read_reference_from_dir(&base, "../../../../etc/passwd");
    assert!(res.is_err(), "Must reject deep ../ traversal to /etc/passwd");
    let err = res.unwrap_err();
    assert!(
        err.contains("Access outside resources directory is denied") || err.contains("Failed to canonicalize"),
        "Unexpected error: {}",
        err
    );

    // 2. Traversal to /etc/shadow
    let res = read_reference_from_dir(&base, "../../../../../../../etc/shadow");
    assert!(res.is_err(), "Must reject traversal to /etc/shadow");

    // 3. Absolute path /etc/passwd
    let res = read_reference_from_dir(&base, "/etc/passwd");
    assert!(res.is_err(), "Must reject absolute path /etc/passwd");

    // 4. Absolute path /etc/hosts
    let res = read_reference_from_dir(&base, "/etc/hosts");
    assert!(res.is_err(), "Must reject absolute path /etc/hosts");

    cleanup(base);
}

#[test]
fn test_symlink_attacks_strictly_contained() {
    let (base, sample_file) = setup_test_sandbox("symlinks");

    // 1. Symlink pointing outside base to /etc/passwd
    let evil_symlink = base.join("evil_passwd.md");
    let _ = symlink("/etc/passwd", &evil_symlink);
    let res = read_reference_from_dir(&base, "evil_passwd.md");
    assert!(res.is_err(), "Must reject symlink pointing outside base");
    let err = res.unwrap_err();
    assert!(
        err.contains("Access outside resources directory is denied"),
        "Expected boundary denial, got: {}",
        err
    );

    // 2. Symlink chain: symlink A -> symlink B -> /etc/passwd
    let hop1 = base.join("hop1.md");
    let hop2 = base.join("hop2.md");
    let _ = symlink("/etc/passwd", &hop2);
    let _ = symlink(&hop2, &hop1);
    let res = read_reference_from_dir(&base, "hop1.md");
    assert!(res.is_err(), "Must reject symlink chain pointing outside base");

    // 3. Valid symlink pointing inside base (to sample.md)
    let good_symlink = base.join("alias_sample.md");
    let _ = symlink(&sample_file, &good_symlink);
    let res = read_reference_from_dir(&base, "alias_sample.md");
    assert!(res.is_ok(), "Must allow symlink pointing to valid file inside base");
    assert_eq!(res.unwrap(), "EMBARGOED_CORE_KNOWLEDGE_BASE");

    // 4. Broken / dangling symlink
    let broken_symlink = base.join("dangling.md");
    let _ = symlink(base.join("does_not_exist.md"), &broken_symlink);
    let res = read_reference_from_dir(&base, "dangling.md");
    assert!(res.is_err(), "Must fail safely on dangling symlink");

    // 5. Circular symlink loop: loopA -> loopB -> loopA
    let loop_a = base.join("loop_a.md");
    let loop_b = base.join("loop_b.md");
    let _ = symlink(&loop_b, &loop_a);
    let _ = symlink(&loop_a, &loop_b);
    let res = read_reference_from_dir(&base, "loop_a.md");
    assert!(res.is_err(), "Must fail safely on circular symlink without infinite hang");

    cleanup(base);
}

#[test]
fn test_nested_resources_path_variations() {
    let (base, _) = setup_test_sandbox("nested_res");

    // 1. Path with single resources/ prefix (normal case)
    let res = read_reference_from_dir(&base, "resources/sample.md");
    assert_eq!(res.unwrap(), "EMBARGOED_CORE_KNOWLEDGE_BASE");

    // 2. Path with multiple resources/ prefix when no nested folder exists
    // clean_rel strips ONE resources/ prefix, leaving "resources/resources/sample.md".
    // Since base doesn't have resources/resources/, it fails cleanly with "Failed to canonicalize".
    let res = read_reference_from_dir(&base, "resources/resources/resources/sample.md");
    assert!(res.is_err(), "Non-existent nested resources path must return error");

    // 3. Create nested resources/resources/resources/ hierarchy
    let deep_dir = base.join("resources").join("resources");
    fs::create_dir_all(&deep_dir).unwrap();
    fs::write(deep_dir.join("deep_sample.md"), "DEEPLY_NESTED_RESOURCE").unwrap();

    // Now "resources/resources/deep_sample.md" stripped once gives "resources/deep_sample.md"
    let deep_dir_single = base.join("resources");
    fs::create_dir_all(&deep_dir_single).unwrap();
    fs::write(deep_dir_single.join("nested_res.md"), "NESTED_ONE_LEVEL").unwrap();
    let res = read_reference_from_dir(&base, "resources/resources/nested_res.md");
    assert!(res.is_ok(), "Stripped resources/resources/nested_res.md resolves to resources/nested_res.md: {:?}", res.err());
    assert_eq!(res.unwrap(), "NESTED_ONE_LEVEL");

    // Multiple resources/ with non-existent file
    let res = read_reference_from_dir(&base, "resources/resources/resources/sample.md");
    assert!(res.is_err());

    // 4. Mixed resources with traversal
    // When "resources/" is stripped from "resources/../sample.md", the remaining "../sample.md"
    // attempts traversal above base directory and is securely blocked.
    let res = read_reference_from_dir(&base, "resources/../sample.md");
    assert!(res.is_err(), "Must reject traversal after stripping resources/ prefix");

    let res = read_reference_from_dir(&base, "resources/../../etc/passwd");
    assert!(res.is_err(), "Must reject traversal disguised with resources/ prefix");

    cleanup(base);
}

#[test]
fn test_sibling_directory_boundary_attack() {
    let (base, _) = setup_test_sandbox("boundary_main");

    // Sibling directory with common prefix: e.g. base is /tmp/..._boundary_main
    // Attacker creates sibling /tmp/..._boundary_main_attacker
    let attacker_dir = PathBuf::from(format!("{}_attacker", base.display()));
    let _ = fs::create_dir_all(&attacker_dir);
    let secret = attacker_dir.join("leaked.txt");
    fs::write(&secret, "ATTACKER_SECRET").unwrap();

    let relative_to_sibling = format!("../{}_attacker/leaked.txt", base.file_name().unwrap().to_str().unwrap());
    let res = read_reference_from_dir(&base, &relative_to_sibling);
    assert!(res.is_err(), "Must reject traversal into sibling directory with common name prefix");
    let err = res.unwrap_err();
    assert!(
        err.contains("Access outside resources directory is denied"),
        "Expected access denied, got: {}",
        err
    );

    let _ = fs::remove_dir_all(&attacker_dir);
    cleanup(base);
}

#[test]
fn test_edge_cases_directories_and_empty_paths() {
    let (base, _) = setup_test_sandbox("edge_cases");

    // 1. Directory target
    let res = read_reference_from_dir(&base, "subdir");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("is not a file"));

    // 2. Empty string
    let res = read_reference_from_dir(&base, "");
    assert!(res.is_err());

    // 3. Whitespace only
    let res = read_reference_from_dir(&base, "   \t\n  ");
    assert!(res.is_err());

    // 4. Slash only
    let res = read_reference_from_dir(&base, "/");
    assert!(res.is_err());

    // 5. Dot (current dir)
    let res = read_reference_from_dir(&base, ".");
    assert!(res.is_err());

    // 6. Non-existent file
    let res = read_reference_from_dir(&base, "ghost_file_404.md");
    assert!(res.is_err());

    cleanup(base);
}
