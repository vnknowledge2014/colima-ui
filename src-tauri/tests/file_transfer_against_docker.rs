//! End-to-end checks for background transfers against a real container runtime.
//!
//! Ignored by default: these need a working Docker daemon, they create and remove
//! a throwaway container, and they write hundreds of megabytes to the temp
//! directory. Run them deliberately:
//!
//! ```text
//! cargo test --test file_transfer_against_docker -- --ignored --test-threads=1
//! ```
//!
//! `--test-threads=1` matters: the transfer registry is process-global and the
//! cancellation test kills by job id.

use colima_ui_lib::commands::file_transfer::{
    cancel_transfer_job, start_copy_from_container, start_copy_to_container, start_image_load,
    start_image_save,
};
use colima_ui_lib::streaming_cmd::{active_stream_count, cancel_stream, partial_sink_path};
use colima_ui_lib::transfer_registry::{self, CancelOutcome, TransferStatus};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Small image, present on the development machine, used for save/load.
const SMALL_IMAGE: &str = "curlimages/curl:latest";
/// Large enough that a cancellation lands mid-write.
const LARGE_IMAGE: &str = "public.ecr.aws/supabase/postgres:17.6.1.158";

/// A scratch directory that removes itself, so a failing assertion cannot leave
/// gigabytes of archives behind. `TMPDIR` is not always the system temp directory
/// — on this project's dev setup it points at the repo — which makes litter both
/// large and in the way.
struct WorkDir(PathBuf);

impl WorkDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("colimaui-e2e-{}", tag));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn str(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("docker must be on PATH for these tests")
}

/// Wait until `f` holds, or fail after `limit`.
fn wait_until(limit: Duration, label: &str, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < limit {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out after {:?} waiting for {}", limit, label);
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn sha256(path: &Path) -> String {
    let out = Command::new("shasum")
        .args(["-a", "256", path.to_str().unwrap()])
        .output()
        .expect("shasum must be available");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Resident memory of this test process, in KB.
fn resident_kb() -> u64 {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps must be available");
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn exported_archive_can_be_loaded_back_by_the_docker_cli() {
    let dir = WorkDir::new("save");
    let started = start_image_save(
        None,
        vec![SMALL_IMAGE.to_string()],
        dir.str().to_string(),
        "exported.tar".to_string(),
        false,
    )
    .expect("export should start");
    assert!(started.total_estimate.unwrap_or(0) > 0, "estimate should be known");

    let tar = dir.join("exported.tar");
    wait_until(Duration::from_secs(180), "export to finish", || {
        active_stream_count() == 0 && file_size(&tar) > 0
    });
    // Give the final flush a moment after the child exits.
    std::thread::sleep(Duration::from_millis(300));

    let size = file_size(&tar);
    assert!(size > 1_000_000, "archive looks too small: {} bytes", size);

    // The criterion is that the docker CLI accepts it, not that our own code does.
    let out = docker(&["load", "-i", tar.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "docker load rejected our archive: {}",
        String::from_utf8_lossy(&out.stderr)
    );

}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn an_archive_written_by_the_docker_cli_can_be_imported() {
    let dir = WorkDir::new("load");
    let tar = dir.join("from-cli.tar");

    // Built by the CLI, not by us — that is the point of this test.
    let out = docker(&["save", "-o", tar.to_str().unwrap(), SMALL_IMAGE]);
    assert!(
        out.status.success(),
        "could not prepare the archive: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    start_image_load(None, tar.to_str().unwrap().to_string()).expect("import should start");
    wait_until(Duration::from_secs(180), "import to finish", || {
        active_stream_count() == 0
    });

    // The image is usable afterwards.
    let out = docker(&["image", "inspect", SMALL_IMAGE]);
    assert!(out.status.success(), "image is not usable after import");

}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn a_hundred_megabyte_file_survives_a_round_trip_through_a_container() {
    let dir = WorkDir::new("cp");
    let source = dir.join("payload.bin");
    let out = Command::new("dd")
        .args([
            "if=/dev/urandom",
            &format!("of={}", source.display()),
            "bs=1048576",
            "count=100",
        ])
        .output()
        .expect("dd must be available");
    assert!(out.status.success(), "could not create the payload");
    assert_eq!(file_size(&source), 100 * 1024 * 1024);
    let expected = sha256(&source);

    // A created-but-never-started container is enough for `docker cp`, and leaves
    // no process running on the developer's machine.
    let name = format!("colimaui-selftest-{}", std::process::id());
    let out = docker(&["create", "--name", &name, SMALL_IMAGE, "true"]);
    assert!(
        out.status.success(),
        "could not create the test container: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let container_id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    let result = std::panic::catch_unwind(|| {
        start_copy_to_container(
            None,
            container_id.clone(),
            source.to_str().unwrap().to_string(),
            "/tmp/payload.bin".to_string(),
        )
        .expect("copy in should start");
        wait_until(Duration::from_secs(300), "copy into the container", || {
            active_stream_count() == 0
        });

        // Copy out produces a TAR archive, not the bare file: the runtime writes a
        // directory when the source is one, which made progress meaningless and the
        // cancel path a silent no-op. One archive covers both shapes, so the
        // extraction below is the price of that and is asserted rather than assumed.
        start_copy_from_container(
            None,
            container_id.clone(),
            "/tmp/payload.bin".to_string(),
            dir.str().to_string(),
            "returned.tar".to_string(),
            false,
        )
        .expect("copy out should start");
        let returned = dir.join("returned.tar");
        wait_until(Duration::from_secs(300), "copy out of the container", || {
            active_stream_count() == 0 && file_size(&returned) > 100 * 1024 * 1024
        });

        // The archive lists the file by its name inside the container.
        let listing = Command::new("tar")
            .args(["-tf", returned.to_str().unwrap()])
            .output()
            .expect("tar must be available");
        assert!(listing.status.success(), "copy out did not produce a readable archive");
        let names = String::from_utf8_lossy(&listing.stdout);
        assert!(
            names.lines().any(|l| l.trim_end_matches('/') == "payload.bin"),
            "archive does not contain payload.bin: {}",
            names
        );

        // Extract and compare content, which is what the round trip is really about.
        let extract = dir.join("extracted");
        std::fs::create_dir_all(&extract).unwrap();
        let out = Command::new("tar")
            .args([
                "-xf",
                returned.to_str().unwrap(),
                "-C",
                extract.to_str().unwrap(),
            ])
            .output()
            .expect("tar must be available");
        assert!(out.status.success(), "could not extract the archive");

        let extracted = extract.join("payload.bin");
        assert_eq!(file_size(&extracted), 100 * 1024 * 1024);
        sha256(&extracted)
    });

    // Clean up before asserting, so a failure does not leave the container behind.
    let _ = docker(&["rm", "-f", &name]);
    let actual = result.expect("round trip panicked");
    assert_eq!(actual, expected, "content changed in transit");

}

/// A client that missed every event can still learn how the transfer ended.
///
/// This is the whole point of the registry: events are lossy and the window that
/// started the job may be gone, so the outcome has to be readable afterwards. The
/// test deliberately subscribes to nothing.
#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn a_finished_transfer_is_still_readable_afterwards() {
    let dir = WorkDir::new("registry");
    let started = start_image_save(
        None,
        vec![SMALL_IMAGE.to_string()],
        dir.str().to_string(),
        "listed.tar".to_string(),
        false,
    )
    .expect("export should start");

    // Visible before it can possibly have finished — registration happens ahead of
    // the spawn precisely so a fast failure is not invisible.
    let found = transfer_registry::list()
        .into_iter()
        .find(|s| s.job_id == started.job_id)
        .expect("a started job must be listed immediately");
    assert_eq!(found.kind, "save");
    assert_eq!(found.target_label, SMALL_IMAGE);

    wait_until(Duration::from_secs(180), "export to finish", || {
        transfer_registry::list()
            .iter()
            .any(|s| s.job_id == started.job_id && s.status == TransferStatus::Success)
    });

    let done = transfer_registry::list()
        .into_iter()
        .find(|s| s.job_id == started.job_id)
        .expect("a finished job must stay readable for the retention window");
    assert!(done.bytes > 1_000_000, "reported {} bytes", done.bytes);
    assert!(done.finished_at.is_some());
    // Success is only recorded once the archive is in place, so this holds.
    assert!(dir.join("listed.tar").is_file());
}

/// A cancel issued immediately after start must still stop the job.
///
/// This is the window the registry exists to close: the caller holds the job id as
/// soon as `start_image_save` returns, but the child is spawned on a blocking task
/// that may not have been scheduled yet. `cancel_stream` alone cannot kill a process
/// that does not exist, so the request is armed and honoured at registration.
#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn a_cancel_issued_before_the_child_exists_still_stops_the_job() {
    let dir = WorkDir::new("cancel-early");
    let started = start_image_save(
        None,
        vec![LARGE_IMAGE.to_string()],
        dir.str().to_string(),
        "never.tar".to_string(),
        false,
    )
    .expect("export should start");

    // No wait at all — the point is to race the spawn.
    assert_eq!(
        cancel_transfer_job(&started.job_id),
        CancelOutcome::Cancelled,
        "a job that has just started must be cancellable"
    );

    wait_until(Duration::from_secs(60), "the job to settle", || {
        transfer_registry::list()
            .iter()
            .any(|s| s.job_id == started.job_id && s.status.is_terminal())
    });

    let settled = transfer_registry::list()
        .into_iter()
        .find(|s| s.job_id == started.job_id)
        .expect("the outcome must be readable");
    assert_eq!(
        settled.status,
        TransferStatus::Cancelled,
        "a cancelled job must not report success"
    );
    assert!(
        !dir.join("never.tar").exists(),
        "a cancelled export must publish nothing"
    );

    // Cancelling again reports the truth rather than a second acknowledgement.
    assert_eq!(
        cancel_transfer_job(&started.job_id),
        CancelOutcome::AlreadyFinished
    );
    assert_eq!(cancel_transfer_job("save-does-not-exist"), CancelOutcome::UnknownJob);
}

/// Two transfers cannot be aimed at one destination while the first is running.
///
/// `resolve_destination` cannot catch this: the first job's output does not exist
/// at that path until it finishes, so the existence check passes for both.
#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn a_second_transfer_cannot_target_a_destination_in_use() {
    let dir = WorkDir::new("conflict");
    let first = start_image_save(
        None,
        vec![LARGE_IMAGE.to_string()],
        dir.str().to_string(),
        "contested.tar".to_string(),
        false,
    )
    .expect("the first export should start");

    let err = start_image_save(
        None,
        vec![SMALL_IMAGE.to_string()],
        dir.str().to_string(),
        "contested.tar".to_string(),
        false,
    )
    .expect_err("the second export must be refused while the first is writing");
    assert!(err.contains(&first.job_id), "unexpected: {}", err);

    cancel_stream(&first.job_id);
    wait_until(Duration::from_secs(30), "the first job to end", || {
        active_stream_count() == 0
    });
}

/// Copying a *directory* out yields one archive rather than a half-written tree.
///
/// This is the shape the old implementation got wrong: `docker cp <id>:<dir> <dest>`
/// made the runtime create a directory at `<dest>`, so progress measured a dirent
/// and cancellation called `remove_file` on a directory and silently did nothing.
#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn a_directory_copies_out_as_a_single_archive() {
    let dir = WorkDir::new("cp-dir");

    let name = format!("colimaui-selftest-dir-{}", std::process::id());
    let out = docker(&[
        "create",
        "--name",
        &name,
        SMALL_IMAGE,
        "sh",
        "-c",
        "true",
    ]);
    assert!(
        out.status.success(),
        "could not create the test container: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let container_id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    // Seed the tree with the CLI rather than `start_copy_to_container`, which takes
    // a single file by design. The subject here is copying *out*.
    let tree = dir.join("tree");
    std::fs::create_dir_all(tree.join("nested")).unwrap();
    std::fs::write(tree.join("a.txt"), b"alpha").unwrap();
    std::fs::write(tree.join("nested/b.txt"), b"beta").unwrap();
    let seeded = docker(&[
        "cp",
        tree.to_str().unwrap(),
        &format!("{}:/tmp/tree", container_id),
    ]);
    assert!(
        seeded.status.success(),
        "could not seed the directory: {}",
        String::from_utf8_lossy(&seeded.stderr)
    );

    let result = std::panic::catch_unwind(|| {
        start_copy_from_container(
            None,
            container_id.clone(),
            "/tmp/tree".to_string(),
            dir.str().to_string(),
            "tree.tar".to_string(),
            false,
        )
        .expect("copy out should start");
        let archive = dir.join("tree.tar");
        wait_until(Duration::from_secs(120), "copy the tree out", || {
            active_stream_count() == 0 && archive.is_file()
        });

        assert!(archive.is_file(), "copy out must produce a file, not a directory");

        let listing = Command::new("tar")
            .args(["-tf", archive.to_str().unwrap()])
            .output()
            .expect("tar must be available");
        assert!(listing.status.success(), "archive is not readable");
        String::from_utf8_lossy(&listing.stdout).to_string()
    });

    let _ = docker(&["rm", "-f", &name]);
    let names = result.expect("directory copy panicked");
    assert!(names.contains("a.txt"), "archive is missing a.txt: {}", names);
    assert!(
        names.contains("b.txt"),
        "archive is missing the nested file: {}",
        names
    );
}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn cancelling_an_export_removes_the_partial_archive_and_leaves_no_orphan() {
    let dir = WorkDir::new("cancel");
    let started = start_image_save(
        None,
        vec![LARGE_IMAGE.to_string()],
        dir.str().to_string(),
        "partial.tar".to_string(),
        false,
    )
    .expect("export should start");
    let tar = dir.join("partial.tar");
    // Bytes land in a scratch sibling and are renamed onto the destination only on
    // success, so an export in flight is watched there.
    let scratch = partial_sink_path(&tar, &started.job_id);

    // Let it get properly under way, so this cancels a real write.
    wait_until(Duration::from_secs(60), "the export to start writing", || {
        file_size(&scratch) > 5 * 1024 * 1024
    });
    let mid_size = file_size(&scratch);
    assert!(mid_size > 0);
    assert!(
        !tar.exists(),
        "the destination must not appear until the export finishes"
    );

    assert!(cancel_stream(&started.job_id), "the job should be cancellable");
    wait_until(Duration::from_secs(15), "the job to end", || {
        active_stream_count() == 0
    });
    std::thread::sleep(Duration::from_millis(400));

    assert!(
        !scratch.exists(),
        "the half-written archive is still there ({} bytes when cancelled)",
        mid_size
    );
    assert!(!tar.exists(), "a cancelled export must publish nothing");

    // No `docker save` left behind. `pgrep -f` covers the grandchild case.
    let out = Command::new("pgrep")
        .args(["-fl", "docker save"])
        .output()
        .expect("pgrep must be available");
    let survivors: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains(LARGE_IMAGE))
        .map(|l| l.to_string())
        .collect();
    assert!(
        survivors.is_empty(),
        "orphaned processes survived cancellation: {:?}",
        survivors
    );

}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn resident_memory_does_not_track_the_exported_image_size() {
    // The reason this feature streams at all. `Command::output()` would hold the
    // entire archive in memory.
    let dir = WorkDir::new("rss");
    let before = resident_kb();

    start_image_save(
        None,
        vec![LARGE_IMAGE.to_string()],
        dir.str().to_string(),
        "big.tar".to_string(),
        false,
    )
    .expect("export should start");

    let tar = dir.join("big.tar");
    let mut peak = before;
    wait_until(Duration::from_secs(600), "the export to finish", || {
        peak = peak.max(resident_kb());
        active_stream_count() == 0 && file_size(&tar) > 0
    });
    std::thread::sleep(Duration::from_millis(300));

    let written = file_size(&tar);
    let growth_kb = peak.saturating_sub(before);

    assert!(written > 100 * 1024 * 1024, "expected a large archive");
    assert!(
        growth_kb < 64 * 1024,
        "RSS grew {} KB while writing {} bytes — the output is being buffered",
        growth_kb,
        written
    );
}
