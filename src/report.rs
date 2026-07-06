//! Report engine — every analyzer feeds one `AuditReport`; renderers consume only it.
//!
//! terminal (comfy-table + owo-colors) · `--md` · `--json` · `--check` exit code.
//! See `docs/architecture.md` §5. Fields fill in as analyzers land across M1/M2.

use comfy_table::{Cell, Color, Table};
use owo_colors::OwoColorize;
use serde::Serialize;

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

    /// Machine-readable output for scripting / CI. Stable shape, decoupled from
    /// internal types via a dedicated DTO.
    pub fn to_json(&self) -> String {
        let s = self.trust_summary();
        let claims = self
            .claims
            .iter()
            .map(|(c, v)| {
                let (verdict, evidence) = verdict_parts(v);
                JsonClaim {
                    kind: kind_tag(&c.kind),
                    claim: claim_label(&c.kind),
                    verdict,
                    evidence,
                    at: c.ts.to_rfc3339(),
                }
            })
            .collect();
        let blast_radius = self
            .blast_radius
            .files
            .iter()
            .map(|f| JsonFile {
                path: &f.path,
                tool: &f.tool,
                in_scope: f.in_scope,
            })
            .collect();
        let dto = JsonReport {
            session: JsonSession {
                path: &self.session.path,
                events: self.session.events,
            },
            trust: JsonTrust {
                verified: s.verified,
                unverified: s.unverified,
                contradicted: s.contradicted,
                scope_pct: s.scope_pct,
            },
            claims,
            blast_radius,
        };
        serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".into())
    }

    /// GitHub-flavored Markdown — paste-ready into a PR comment.
    pub fn to_markdown(&self) -> String {
        let s = self.trust_summary();
        let mut out = String::from("## what did you do?\n\n");
        out.push_str(&format!(
            "`{}` · {} events\n\n",
            self.session.path, self.session.events
        ));
        out.push_str(&format!(
            "**{} verified · {} unverified · {} contradicted · scope {}%**\n\n",
            s.verified, s.unverified, s.contradicted, s.scope_pct
        ));

        out.push_str("### Claims\n\n");
        if self.claims.is_empty() {
            out.push_str("_no checkable claims found_\n\n");
        } else {
            out.push_str("| Claim | Verdict | Evidence |\n|---|---|---|\n");
            for (c, v) in &self.claims {
                let (tag, evidence) = verdict_parts(v);
                out.push_str(&format!(
                    "| {} | {} {} | {} |\n",
                    claim_label(&c.kind),
                    verdict_emoji(v),
                    tag,
                    md_escape(evidence),
                ));
            }
            out.push('\n');
        }

        out.push_str("### Blast radius\n\n");
        if self.blast_radius.files.is_empty() {
            out.push_str("_no files written via Write/Edit_\n");
        } else {
            out.push_str("| File | Tool | Scope |\n|---|---|---|\n");
            for f in &self.blast_radius.files {
                let scope = if f.in_scope {
                    "in scope"
                } else {
                    "**out of scope**"
                };
                out.push_str(&format!(
                    "| {} | {} | {} |\n",
                    md_escape(&f.path),
                    f.tool,
                    scope
                ));
            }
        }
        out
    }
}

// ---- JSON DTO (stable wire format, independent of domain types) ----

#[derive(Serialize)]
struct JsonReport<'a> {
    session: JsonSession<'a>,
    trust: JsonTrust,
    claims: Vec<JsonClaim<'a>>,
    blast_radius: Vec<JsonFile<'a>>,
}

#[derive(Serialize)]
struct JsonSession<'a> {
    path: &'a str,
    events: usize,
}

#[derive(Serialize)]
struct JsonTrust {
    verified: usize,
    unverified: usize,
    contradicted: usize,
    scope_pct: u8,
}

#[derive(Serialize)]
struct JsonClaim<'a> {
    kind: &'static str,
    claim: String,
    verdict: &'static str,
    evidence: &'a str,
    at: String,
}

#[derive(Serialize)]
struct JsonFile<'a> {
    path: &'a str,
    tool: &'a str,
    in_scope: bool,
}

fn verdict_parts(v: &Verdict) -> (&'static str, &str) {
    match v {
        Verdict::Verified(e) => ("Verified", e.as_str()),
        Verdict::Unverified(e) => ("Unverified", e.as_str()),
        Verdict::Contradicted(e) => ("Contradicted", e.as_str()),
    }
}

fn verdict_emoji(v: &Verdict) -> &'static str {
    match v {
        Verdict::Verified(_) => "✅",
        Verdict::Unverified(_) => "⚠️",
        Verdict::Contradicted(_) => "❌",
    }
}

fn kind_tag(kind: &ClaimKind) -> &'static str {
    match kind {
        ClaimKind::TestsPass => "TestsPass",
        ClaimKind::BuildSucceeds => "BuildSucceeds",
        ClaimKind::FileCreated(_) => "FileCreated",
        ClaimKind::BugFixed => "BugFixed",
        ClaimKind::Committed => "Committed",
    }
}

/// Escape the pipe so evidence/paths can't break a Markdown table row.
fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
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
