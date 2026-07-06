//! Report engine — every analyzer feeds one `AuditReport`; renderers consume only it.
//!
//! terminal (comfy-table + owo-colors) · `--md` · `--json` · `--check` exit code.
//! See `docs/architecture.md` §5. Fields fill in as analyzers land across M1/M2.

use comfy_table::{Cell, Color, Table};
use owo_colors::OwoColorize;

use crate::analyzers::blast_radius::BlastRadius;

/// Identifying facts about the audited session, for the report header.
#[derive(Debug, Default)]
pub struct SessionMeta {
    pub path: String,
    pub events: usize,
}

/// The single struct all analyzers feed and all renderers consume.
#[derive(Debug, Default)]
pub struct AuditReport {
    pub session: SessionMeta,
    pub blast_radius: BlastRadius,
    // claims, dependencies, hygiene, trust_summary — added across M1/M2.
}

impl AuditReport {
    /// Screenshot-friendly terminal rendering.
    pub fn to_terminal(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}\n", "what did you do?".bold()));
        out.push_str(&format!(
            "session {} · {} events\n\n",
            self.session.path.dimmed(),
            self.session.events
        ));

        out.push_str(&format!("{}\n", "Blast radius".bold()));
        if self.blast_radius.files.is_empty() {
            out.push_str(&format!("{}\n", "  no files written".dimmed()));
            return out;
        }

        let mut table = Table::new();
        table.set_header(vec!["File", "Tool", "Scope"]);
        for f in &self.blast_radius.files {
            let (label, color) = if f.in_scope {
                ("in scope", Color::Green)
            } else {
                ("OUT OF SCOPE", Color::Red)
            };
            table.add_row(vec![
                Cell::new(&f.path),
                Cell::new(&f.tool),
                Cell::new(label).fg(color),
            ]);
        }
        out.push_str(&table.to_string());
        out.push('\n');

        let oos = self.blast_radius.out_of_scope_count();
        let summary = format!(
            "{} file(s) written · {} out of scope",
            self.blast_radius.files.len(),
            oos
        );
        out.push_str(&format!(
            "\n{}\n",
            if oos > 0 {
                summary.red().to_string()
            } else {
                summary.green().to_string()
            }
        ));
        out
    }
}
