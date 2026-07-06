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
