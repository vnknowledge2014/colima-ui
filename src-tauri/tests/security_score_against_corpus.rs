//! The calibration gate for the scoring formula.
//!
//! A score that gives every image 40-50 is not a score, it is decoration. This
//! runs the real pipeline over whatever images the machine already has and
//! checks that the numbers actually separate them — and prints the table, so a
//! human can see whether the ordering matches what they would have said.
//!
//! ```text
//! cargo test --test security_score_against_corpus -- --ignored --nocapture
//! ```
//!
//! Ignored by default: needs Trivy, a container runtime, and minutes of scanning.

use colima_ui_lib::commands::security_rules::{collect_facts_blocking, evaluate, Level};
use colima_ui_lib::commands::security_scan::scan_image_blocking;
use colima_ui_lib::commands::security_score::score;

/// Enough images to see a distribution without turning the suite into a scan farm.
const MAX_IMAGES: usize = 12;

fn have(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn local_images() -> Vec<String> {
    let out = std::process::Command::new("docker")
        .args(["images", "--format", "{{.Repository}}:{{.Tag}}"])
        .output()
        .expect("docker images");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.contains("<none>"))
        .take(MAX_IMAGES)
        .collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[test]
#[ignore = "needs Trivy, a container runtime, and minutes of scanning"]
fn the_score_separates_images_instead_of_bunching_them() {
    if !have("trivy", &["--version"]) || !have("docker", &["info"]) {
        eprintln!("skipping: needs trivy and a container runtime");
        return;
    }

    let images = local_images();
    if images.len() < 3 {
        eprintln!("skipping: need at least three local images, found {}", images.len());
        return;
    }

    let now = now_ms();
    let mut totals = Vec::new();

    println!(
        "\n{:<48} {:>8} {:>5} {:>5} {:>5} {:>5} {:>6}",
        "image", "weighted", "vuln", "hard", "prov", "fresh", "total"
    );
    println!("{}", "-".repeat(88));

    for (i, image) in images.iter().enumerate() {
        let scan = match scan_image_blocking(&format!("corpus-{}", i), image, false) {
            Ok(s) => s,
            // A scanner that cannot read one image is a normal result — it must
            // not take the rest of the corpus with it.
            Err(e) => {
                println!("{:<52} skipped: {}", image, e.chars().take(60).collect::<String>());
                continue;
            }
        };
        let facts = match collect_facts_blocking(image) {
            Ok(f) => f,
            Err(e) => {
                println!("{:<52} skipped: {}", image, e.chars().take(60).collect::<String>());
                continue;
            }
        };
        let evaluation = evaluate(&facts, Level::L1, now);
        let s = score(&scan, &evaluation, Level::L1);

        println!(
            "{:<48} {:>8.0} {:>5} {:>5} {:>5} {:>5} {:>6}",
            image.chars().take(48).collect::<String>(),
            colima_ui_lib::commands::security_score::weighted_findings(&scan),
            s.vulnerabilities.earned,
            s.hardening.earned,
            s.provenance.earned,
            s.freshness.earned,
            s.total
        );
        totals.push((image.clone(), s.total));
    }

    assert!(totals.len() >= 3, "not enough images scored to judge a distribution");

    let min = totals.iter().map(|(_, t)| *t).min().unwrap();
    let max = totals.iter().map(|(_, t)| *t).max().unwrap();
    let distinct: std::collections::HashSet<u32> = totals.iter().map(|(_, t)| *t).collect();

    println!("\nspread {}..{} over {} images, {} distinct values", min, max, totals.len(), distinct.len());

    // The gate. If every image lands within a few points of every other, the
    // formula is not measuring anything and the weights need changing before
    // this ships — which is the whole reason this test exists.
    assert!(
        max - min >= 20,
        "scores bunched between {} and {}: the formula does not distinguish images",
        min,
        max
    );
    assert!(
        distinct.len() >= 3,
        "only {} distinct scores across {} images",
        distinct.len(),
        totals.len()
    );
}

#[test]
#[ignore = "needs Trivy and a container runtime"]
fn a_pinned_non_root_image_beats_a_root_image_on_a_moving_tag() {
    if !have("trivy", &["--version"]) || !have("docker", &["info"]) {
        eprintln!("skipping: needs trivy and a container runtime");
        return;
    }

    // The success criterion from the phase file, executable. Both images are
    // scanned the same way; the difference is entirely configuration.
    let good = "node:20-alpine";
    let bad = "node:latest";
    for image in [good, bad] {
        let pulled = std::process::Command::new("docker")
            .args(["image", "inspect", image])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !pulled {
            eprintln!("skipping: {} is not present locally", image);
            return;
        }
    }

    let now = now_ms();
    let audit = |image: &str, id: &str| {
        let scan = scan_image_blocking(id, image, false).expect("scan");
        let facts = collect_facts_blocking(image).expect("facts");
        let evaluation = evaluate(&facts, Level::L1, now);
        score(&scan, &evaluation, Level::L1)
    };

    let g = audit(good, "cmp-good");
    let b = audit(bad, "cmp-bad");
    println!("{} = {}   {} = {}", good, g.total, bad, b.total);

    assert!(
        g.total > b.total,
        "a pinned, smaller, non-root image must not score below {}",
        bad
    );
}
