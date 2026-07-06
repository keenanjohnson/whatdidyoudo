//! Report engine — every analyzer feeds one `AuditReport`; renderers consume only it.
//!
//! terminal (comfy-table + owo-colors) · `--md` · `--json` · `--check` exit code.
//! See `docs/architecture.md` §5. Fields fill in as analyzers land across M1/M2.

use comfy_table::{Cell, Color, Table};
use owo_colors::OwoColorize;

use crate::analyzers::blast_radius::BlastRadius;
use crate::analyzers::claims::Claim;
use crate::analyzers::{ClaimKind, Verdict};

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
    pub claims: Vec<(Claim, Verdict)>,
    // dependencies, hygiene — added later in M2.
}

/// The one-line punchline: how many claims held up, and how much work stayed in scope.
pub struct TrustSummary {
    pub verified: usize,
    pub unverified: usize,
    pub contradicted: usize,
    pub scope_pct: u8,
}

impl AuditReport {
    pub fn trust_summary(&self) -> TrustSummary {
        let mut s = TrustSummary {
            verified: 0,
            unverified: 0,
            contradicted: 0,
            scope_pct: scope_pct(&self.blast_radius),
        };
        for (_, verdict) in &self.claims {
            match verdict {
                Verdict::Verified(_) => s.verified += 1,
                Verdict::Unverified(_) => s.unverified += 1,
                Verdict::Contradicted(_) => s.contradicted += 1,
            }
        }
        s
    }

    /// Screenshot-friendly terminal rendering.
    pub fn to_terminal(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}\n", "what did you do?".bold()));
        out.push_str(&format!(
            "session {} · {} events\n\n",
            self.session.path.dimmed(),
            self.session.events
        ));

        out.push_str(&self.render_claims());
        out.push('\n');
        out.push_str(&self.render_blast_radius());
        out.push('\n');
        out.push_str(&self.render_summary());
        out
    }

    fn render_claims(&self) -> String {
        let mut out = format!("{}\n", "Claims".bold());
        if self.claims.is_empty() {
            out.push_str(&format!("{}\n", "  no checkable claims found".dimmed()));
            return out;
        }
        let mut table = Table::new();
        table.set_header(vec!["Claim", "Verdict", "Evidence"]);
        for (claim, verdict) in &self.claims {
            let (label, color, evidence) = match verdict {
                Verdict::Verified(e) => ("VERIFIED", Color::Green, e.as_str()),
                Verdict::Unverified(e) => ("UNVERIFIED", Color::Yellow, e.as_str()),
                Verdict::Contradicted(e) => ("CONTRADICTED", Color::Red, e.as_str()),
            };
            table.add_row(vec![
                Cell::new(claim_label(&claim.kind)),
                Cell::new(label).fg(color),
                Cell::new(evidence),
            ]);
        }
        out.push_str(&table.to_string());
        out.push('\n');
        out
    }

    fn render_blast_radius(&self) -> String {
        let mut out = format!("{}\n", "Blast radius".bold());
        if self.blast_radius.files.is_empty() {
            out.push_str(&format!(
                "{}\n",
                "  no files written via Write/Edit".dimmed()
            ));
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
        out
    }

    fn render_summary(&self) -> String {
        let s = self.trust_summary();
        let text = format!(
            "{} verified · {} unverified · {} contradicted · scope {}%",
            s.verified, s.unverified, s.contradicted, s.scope_pct
        );
        // Red if anything was contradicted — that's the headline failure.
        let colored = if s.contradicted > 0 {
            text.red().bold().to_string()
        } else if s.unverified > 0 {
            text.yellow().to_string()
        } else {
            text.green().to_string()
        };
        format!("{colored}\n")
    }
}

/// Percent of written files that were in scope; 100% when nothing was written.
fn scope_pct(br: &BlastRadius) -> u8 {
    if br.files.is_empty() {
        return 100;
    }
    let in_scope = br.files.iter().filter(|f| f.in_scope).count();
    ((in_scope * 100) / br.files.len()) as u8
}

/// Short canonical label naming what the agent claimed.
fn claim_label(kind: &ClaimKind) -> String {
    match kind {
        ClaimKind::TestsPass => "tests pass".into(),
        ClaimKind::BuildSucceeds => "build succeeds".into(),
        ClaimKind::FileCreated(p) => format!("created {}", p.display()),
        ClaimKind::BugFixed => "bug fixed".into(),
        ClaimKind::Committed => "committed".into(),
    }
}
