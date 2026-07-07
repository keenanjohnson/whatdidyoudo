//! Analyzer regression: the blast-radius pass over a parsed fixture timeline.

use std::fs::File;
use std::io::BufReader;

use whatdidyoudo::analyzers::blast_radius::{self, FileTouch};
use whatdidyoudo::{ClaudeCodeAdapter, SourceAdapter};

fn events(name: &str) -> Vec<whatdidyoudo::Event> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    ClaudeCodeAdapter::parse(BufReader::new(File::open(path).unwrap())).collect()
}

#[test]
fn blast_radius_finds_the_written_file_in_scope() {
    // The user named src/hello.rs; the agent wrote exactly that via the Write tool.
    let br = blast_radius::analyze(&events("session_typical.jsonl"));
    assert_eq!(
        br.files,
        vec![FileTouch {
            path: "src/hello.rs".to_string(),
            tool: "Write".to_string(),
            in_scope: true,
        }]
    );
    assert_eq!(br.out_of_scope_count(), 0);
}

#[test]
fn blast_radius_empty_for_noise_session() {
    let br = blast_radius::analyze(&events("session_noise.jsonl"));
    assert!(br.files.is_empty());
}

#[test]
fn task_naming_a_file_is_not_broad() {
    // "Add a greet() function in src/hello.rs …" names a path — scope is judgeable.
    let br = blast_radius::analyze(&events("session_typical.jsonl"));
    assert!(!br.broad_task);
}

#[test]
fn task_naming_no_files_is_broad() {
    // "Finish the discovery module and commit it." names nothing — this session
    // scored a hostile 0% scope before broad-task detection.
    let br = blast_radius::analyze(&events("session_false_verdicts.jsonl"));
    assert!(br.broad_task);
}

#[test]
fn prose_abbreviations_do_not_defeat_broad_task_detection() {
    use whatdidyoudo::Timestamp;
    let ev = vec![whatdidyoudo::Event::UserMessage {
        ts: Timestamp::from_unix(1).unwrap(),
        text: "Clean up the error handling, i.e. stop panicking on bad input.".into(),
    }];
    assert!(blast_radius::analyze(&ev).broad_task);
}

#[test]
fn git_changes_merge_without_duplicating_tool_writes() {
    use std::path::PathBuf;
    let ev = events("session_typical.jsonl"); // wrote src/hello.rs via Write
    let mut br = blast_radius::analyze(&ev);

    // src/hello.rs came from git too (dedup); scripts/deploy.sh is a shell-only edit.
    let changed = vec![
        PathBuf::from("src/hello.rs"),
        PathBuf::from("scripts/deploy.sh"),
    ];
    blast_radius::merge_git_changes(&mut br, &ev, &changed);

    assert_eq!(
        br.files
            .iter()
            .filter(|f| f.path.ends_with("hello.rs"))
            .count(),
        1,
        "the Write and the git change should not double-count"
    );
    let shell = br
        .files
        .iter()
        .find(|f| f.path == "scripts/deploy.sh")
        .unwrap();
    assert_eq!(shell.tool, "shell/git");
    assert!(!shell.in_scope, "deploy.sh was never mentioned by the user");
}
