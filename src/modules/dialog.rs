use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use super::Module;
use crate::db::{Concept, ConfidenceUpdate, Database, Evidence};

#[derive(Clone, Debug)]
struct Proposal {
    name: String,
    definition: String,
    confidence: f64,
}

#[derive(Clone, Debug)]
enum PendingAction {
    Concept(Proposal),
    Relation {
        from: String,
        relation_type: String,
        to: String,
    },
    Episode {
        outcome: String,
        summary: String,
    },
    Evidence {
        concept: String,
        content: String,
        source: Option<String>,
        domain: Option<String>,
    },
    TagEpisode {
        episode_id: i64,
        tags: Vec<String>,
        outcome: String,
    },
    Recalc {
        updates: Vec<ConfidenceUpdate>,
    },
    TrustAdjust {
        evidence_id: i64,
        direction: String,
    },
    SkillNew {
        name: String,
        description: String,
    },
    SkillAdd {
        name: String,
        text: String,
    },
    Suggestion {
        plans: Vec<String>,
    },
}

pub struct Dialog {
    input: String,
    history: Vec<String>,
    db: Database,
    pending: Option<PendingAction>,
    ollama_available: bool,
}

impl Dialog {
    pub fn new(db: Database) -> Self {
        let ollama_available = std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v ollama >/dev/null 2>&1")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        let mut history = vec![
            "MOTHER: DIALOG READY.".into(),
            "MOTHER: Commands:".into(),
            "  learn <concept> is <definition>".into(),
            "  rel <from> <type> <to>".into(),
            "  ep ok|fail|note <summary>".into(),
            "  episodes [<concept>]".into(),
            "  evidence <concept> :: <content> [:: <source>]".into(),
            "  trust <evidence_id> up|down".into(),
            "  show <concept> | list | recalc | gaps".into(),
            "  skill new <name> :: <desc> | skill add <name> :: <step> | skill show/run <name>"
                .into(),
            "MOTHER: If a proposal appears: press [y] to confirm, [n] to reject.".into(),
        ];
        if !ollama_available {
            history.push(
                "MOTHER: Local model (Ollama) not detected; proposals will skip model use.".into(),
            );
        }

        Self {
            input: String::new(),
            history,
            db,
            pending: None,
            ollama_available,
        }
    }

    fn push(&mut self, line: impl Into<String>) {
        self.history.push(line.into());
        if self.history.len() > 240 {
            self.history.drain(0..70);
        }
    }

    fn eliza_reflect(&self, text: &str) -> String {
        format!("MOTHER: Why do you say '{}'?", &text)
    }

    fn handle_command(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        // If a proposal is pending, require resolution first.
        if self.pending.is_some() {
            self.push("MOTHER: Resolve current proposal first ([y]/[n]).");
            return;
        }

        // recalc confidences
        if trimmed.eq_ignore_ascii_case("recalc") {
            match self.db.calculate_confidence_updates() {
                Ok(updates) if updates.is_empty() => {
                    self.push("MOTHER: Confidence already up-to-date.")
                }
                Ok(updates) => {
                    self.pending = Some(PendingAction::Recalc {
                        updates: updates.clone(),
                    });
                    self.push("MOTHER: PROPOSAL: apply confidence recalculation.");
                    for u in updates.iter().take(10) {
                        self.push(format!("  {}: {:.2} -> {:.2}", u.concept, u.old, u.new));
                    }
                    if updates.len() > 10 {
                        self.push(format!("  ...and {} more", updates.len() - 10));
                    }
                    self.push("Confirm? [y]/[n]");
                }
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // gaps
        if trimmed.eq_ignore_ascii_case("gaps") {
            match self.gaps_report() {
                Some(plans) => {
                    self.pending = Some(PendingAction::Suggestion {
                        plans: plans.clone(),
                    });
                    self.push("MOTHER: GAP PROPOSALS:");
                    for plan in plans.iter() {
                        self.push(format!("  PLAN: {}", plan));
                    }
                    self.push("Accept proposed plans? (no execution) [y]/[n]");
                }
                None => self.push("MOTHER: No gaps detected."),
            }
            return;
        }

        // episodes listing / filtering
        if trimmed.to_lowercase().starts_with("episodes") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() == 1 {
                self.show_recent_episodes(None);
            } else {
                let concept = parts[1].to_lowercase();
                self.show_recent_episodes(Some(&concept));
            }
            return;
        }

        // ep <ok|fail|note> <summary>
        if let Some(rest) = trimmed.strip_prefix("ep ") {
            let mut parts = rest.splitn(2, ' ');
            let outcome = parts.next().unwrap_or("").trim().to_lowercase();
            let summary = parts.next().unwrap_or("").trim().to_string();

            let valid = outcome == "ok" || outcome == "fail" || outcome == "note";
            if !valid || summary.is_empty() {
                self.push("MOTHER: Format is: ep ok <what worked> | ep fail <what failed> | ep note <note>");
                return;
            }

            self.pending = Some(PendingAction::Episode {
                outcome: outcome.clone(),
                summary: summary.clone(),
            });
            self.push(format!(
                "MOTHER: PROPOSAL: store episode [{}] {}",
                outcome, summary
            ));
            self.push("Confirm? [y]/[n]");
            return;
        }

        // evidence command
        if let Some(rest) = trimmed.strip_prefix("evidence ") {
            let parts: Vec<&str> = rest.split("::").collect();
            if parts.len() < 2 {
                self.push("MOTHER: Format: evidence <concept> :: <content> [:: <source>]");
                return;
            }
            let concept = parts[0].trim().to_lowercase();
            let content = parts[1].trim().to_string();
            let source = parts
                .get(2)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            if concept.is_empty() || content.is_empty() {
                self.push("MOTHER: concept and content required.");
                return;
            }

            match self.db.get_concept(&concept) {
                Ok(Some(_)) => {
                    let domain = source.as_ref().and_then(|s| Self::derive_domain(s));
                    self.pending = Some(PendingAction::Evidence {
                        concept: concept.clone(),
                        content: content.clone(),
                        source,
                        domain,
                    });
                    self.push("MOTHER: PROPOSAL: attach evidence");
                    self.push(format!("  Concept: {}", concept));
                    self.push(format!("  Content: {}", content));
                    self.push("Confirm? [y]/[n]");
                }
                Ok(None) => self.push(format!("MOTHER: No known concept '{}'.", concept)),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // trust command
        if let Some(rest) = trimmed.strip_prefix("trust ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 2 {
                self.push("MOTHER: trust format: trust <evidence_id> up|down");
                return;
            }
            let id: i64 = match parts[0].parse() {
                Ok(v) => v,
                Err(_) => {
                    self.push("MOTHER: evidence id must be a number.");
                    return;
                }
            };
            let direction = parts[1].to_lowercase();
            if direction != "up" && direction != "down" {
                self.push("MOTHER: trust direction must be up|down.");
                return;
            }
            self.pending = Some(PendingAction::TrustAdjust {
                evidence_id: id,
                direction: direction.clone(),
            });
            self.push(format!("MOTHER: PROPOSAL: trust {} {}", id, direction));
            self.push("Confirm? [y]/[n]");
            return;
        }

        // list concepts
        if trimmed.eq_ignore_ascii_case("list") {
            match self.db.list_concepts(20) {
                Ok(items) if items.is_empty() => self.push("MOTHER: No concepts stored yet."),
                Ok(items) => {
                    self.push("MOTHER: Recent concepts:");
                    for c in items {
                        self.push(format!("  - {} (conf {:.2})", c.name, c.confidence));
                    }
                }
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // show <concept>
        if let Some(rest) = trimmed.strip_prefix("show ") {
            let name = rest.trim().to_lowercase();
            match self.db.get_concept(&name) {
                Ok(Some(c)) => self.show_concept(&c),
                Ok(None) => self.push(format!("MOTHER: I have no concept named '{}'.", name)),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // learn <concept> is <definition>
        if let Some(rest) = trimmed.strip_prefix("learn ") {
            let parts: Vec<&str> = rest.splitn(2, " is ").collect();
            if parts.len() != 2 {
                self.push("MOTHER: Format is: learn <concept> is <definition>");
                return;
            }

            let name = parts[0].trim().to_lowercase();
            let definition = parts[1].trim().to_string();

            if name.is_empty() || definition.is_empty() {
                self.push("MOTHER: Concept name and definition must be non-empty.");
                return;
            }

            self.pending = Some(PendingAction::Concept(Proposal {
                name: name.clone(),
                definition: definition.clone(),
                confidence: 0.40,
            }));

            self.push("MOTHER: PROPOSAL CREATED.");
            self.push(format!("  Concept: {}", name));
            self.push(format!("  Definition: {}", definition));
            self.push("MOTHER: Confirm? [y]es / [n]o");
            return;
        }

        // rel <from> <type> <to>
        if let Some(rest) = trimmed.strip_prefix("rel ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() < 3 {
                self.push("MOTHER: Format is: rel <from> <type> <to>");
                self.push("MOTHER: Example: rel jwt used_for authentication");
                return;
            }
            let from = parts[0].trim().to_lowercase();
            let relation_type = parts[1].trim().to_lowercase();
            let to = parts[2..].join(" ").trim().to_lowercase();

            if from.is_empty() || relation_type.is_empty() || to.is_empty() {
                self.push("MOTHER: rel fields must be non-empty.");
                return;
            }

            self.pending = Some(PendingAction::Relation {
                from: from.clone(),
                relation_type: relation_type.clone(),
                to: to.clone(),
            });
            self.push(format!(
                "MOTHER: PROPOSAL: link {} --{}--> {}",
                from, relation_type, to
            ));
            self.push("Confirm? [y]/[n]");
            return;
        }

        // skills
        if let Some(rest) = trimmed.strip_prefix("skill ") {
            let lower = rest.to_lowercase();
            if let Some(cmd) = lower.strip_prefix("new ") {
                let parts: Vec<&str> = cmd.split("::").collect();
                if parts.len() != 2 {
                    self.push("MOTHER: skill new <name> :: <description>");
                    return;
                }
                let name = parts[0].trim().to_lowercase();
                let desc = parts[1].trim().to_string();
                if name.is_empty() || desc.is_empty() {
                    self.push("MOTHER: skill name and description required.");
                    return;
                }
                self.pending = Some(PendingAction::SkillNew {
                    name: name.clone(),
                    description: desc.clone(),
                });
                self.push(format!(
                    "MOTHER: PROPOSAL: new skill '{}' :: {}",
                    name, desc
                ));
                self.push("Confirm? [y]/[n]");
                return;
            }

            if let Some(cmd) = lower.strip_prefix("add ") {
                let parts: Vec<&str> = cmd.split("::").collect();
                if parts.len() != 2 {
                    self.push("MOTHER: skill add <name> :: <step>");
                    return;
                }
                let name = parts[0].trim().to_lowercase();
                let step = parts[1].trim().to_string();
                if name.is_empty() || step.is_empty() {
                    self.push("MOTHER: skill name and step required.");
                    return;
                }
                self.pending = Some(PendingAction::SkillAdd {
                    name: name.clone(),
                    text: step.clone(),
                });
                self.push(format!(
                    "MOTHER: PROPOSAL: append skill '{}' step: {}",
                    name, step
                ));
                self.push("Confirm? [y]/[n]");
                return;
            }

            if let Some(name) = lower.strip_prefix("show ") {
                let name = name.trim().to_lowercase();
                self.show_skill(&name, false);
                return;
            }

            if let Some(name) = lower.strip_prefix("run ") {
                let name = name.trim().to_lowercase();
                self.show_skill(&name, true);
                return;
            }

            self.push("MOTHER: skill commands: new | add | show | run");
            return;
        }

        if trimmed.eq_ignore_ascii_case("model status") {
            if self.ollama_available {
                self.push("MOTHER: Local model detected. Outputs will remain proposals requiring approval.");
            } else {
                self.push("MOTHER: No local model detected; skipping model suggestions.");
            }
            return;
        }

        // fallback
        self.push(self.eliza_reflect(trimmed));
    }

    fn show_recent_episodes(&mut self, concept: Option<&str>) {
        let fetch = if let Some(c) = concept {
            self.db.list_episodes_for_concept(c, 20)
        } else {
            self.db.list_episodes(20)
        };

        match fetch {
            Ok(items) if items.is_empty() => self.push("MOTHER: No episodes stored yet."),
            Ok(items) => {
                if let Some(c) = concept {
                    self.push(format!("MOTHER: Episodes tagged with '{}':", c));
                } else {
                    self.push("MOTHER: Recent episodes:");
                }
                for e in items {
                    self.push(format!(
                        "  - [{}] #{} {}  {}",
                        e.outcome, e.id, e.captured_at, e.summary
                    ));
                    if let Ok(tags) = self.db.list_episode_tags(e.id) {
                        if !tags.is_empty() {
                            self.push(format!("    tags: {}", tags.join(", ")));
                        }
                    }
                }
            }
            Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
        }
    }

    fn show_concept(&mut self, c: &Concept) {
        self.push("MOTHER: CONCEPT RECORD");
        self.push(format!("  Name: {}", c.name));
        self.push(format!("  Definition: {}", c.definition));
        self.push(format!("  Confidence: {:.2}", c.confidence));
        self.push(format!("  Created: {}", c.created_at));
        if let Ok(history) = self.db.concept_confidence_history(&c.name) {
            if !history.is_empty() {
                self.push("  Events:");
                for (evt, ts) in history {
                    self.push(format!("    - {} @ {}", evt, ts));
                }
            }
        }
    }

    fn show_skill(&mut self, name: &str, run: bool) {
        match self.db.get_skill(name) {
            Ok(Some(skill)) => {
                self.push(format!(
                    "MOTHER: Skill '{}': {}",
                    skill.name, skill.description
                ));
                match self.db.list_skill_steps(skill.id) {
                    Ok(steps) if steps.is_empty() => self.push("  (no steps yet)"),
                    Ok(steps) => {
                        for step in steps {
                            let prefix = if run { ">>" } else { "--" };
                            self.push(format!("  {} [{}] {}", prefix, step.step_no, step.text));
                        }
                    }
                    Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
                }
            }
            Ok(None) => self.push(format!("MOTHER: No skill named '{}'", name)),
            Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
        }
    }

    fn confirm_pending(&mut self) {
        let pending = self.pending.take();
        match pending {
            Some(PendingAction::Concept(p)) => {
                match self.db.upsert_concept(&p.name, &p.definition, p.confidence) {
                    Ok(()) => {
                        let _ = self.db.record_confidence_event(&p.name, "confirm_claim");
                        self.push("MOTHER: COMMITTED.");
                        self.push(format!("  Stored concept '{}'.", p.name));
                    }
                    Err(e) => self.push(format!("MOTHER: DB error committing proposal: {}", e)),
                }
            }
            Some(PendingAction::Relation {
                from,
                relation_type,
                to,
            }) => match self.db.upsert_relation(&from, &relation_type, &to) {
                Ok(()) => self.push(format!(
                    "MOTHER: Linked {} --{}--> {}",
                    from, relation_type, to
                )),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            },
            Some(PendingAction::Episode { outcome, summary }) => {
                match self.db.add_episode(&outcome, &summary) {
                    Ok(id) => {
                        self.push(format!(
                            "MOTHER: EPISODE RECORDED [{}] #{} {}",
                            outcome, id, summary
                        ));
                        if let Some(tags) = self.suggest_tags(&summary) {
                            self.pending = Some(PendingAction::TagEpisode {
                                episode_id: id,
                                tags: tags.clone(),
                                outcome: outcome.clone(),
                            });
                            self.push("MOTHER: Proposed tags for episode:");
                            for t in tags {
                                self.push(format!("  - {}", t));
                            }
                            self.push("Apply tags? [y]/[n]");
                        }
                    }
                    Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
                }
            }
            Some(PendingAction::Evidence {
                concept,
                content,
                source,
                domain,
            }) => {
                match self
                    .db
                    .add_evidence(&concept, &content, source.clone(), domain.clone())
                {
                    Ok(id) => {
                        let _ = self.db.record_confidence_event(&concept, "confirm_claim");
                        self.push(format!("MOTHER: Evidence stored #{} for {}", id, concept));
                        if let Some(src) = source {
                            self.push(format!("  Source: {}", src));
                        }
                        if let Some(dom) = domain {
                            self.push(format!("  Domain: {}", dom));
                        }
                    }
                    Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
                }
            }
            Some(PendingAction::TagEpisode {
                episode_id,
                tags,
                outcome,
            }) => {
                for tag in tags.iter() {
                    if let Err(e) = self.db.add_episode_tag(episode_id, tag) {
                        self.push(format!("MOTHER: Tag '{}' failed: {}", tag, e));
                        continue;
                    }
                    let evt = if outcome == "ok" {
                        "episode_ok"
                    } else if outcome == "fail" {
                        "episode_fail"
                    } else {
                        "episode_note"
                    };
                    let _ = self.db.record_confidence_event(tag, evt);
                }
                self.push(format!("MOTHER: Tags applied to episode #{}.", episode_id));
            }
            Some(PendingAction::Recalc { updates }) => {
                match self.db.apply_confidence_updates(&updates) {
                    Ok(()) => {
                        self.push("MOTHER: Confidence recalculated.");
                        for u in updates.iter().take(10) {
                            self.push(format!("  {}: {:.2} -> {:.2}", u.concept, u.old, u.new));
                        }
                        if updates.len() > 10 {
                            self.push(format!("  ...and {} more", updates.len() - 10));
                        }
                    }
                    Err(e) => self.push(format!("MOTHER: DB error applying recalculation: {}", e)),
                }
            }
            Some(PendingAction::TrustAdjust {
                evidence_id,
                direction,
            }) => match self.db.adjust_trust(evidence_id, &direction) {
                Ok(Some(ev)) => self.render_evidence_update(&ev),
                Ok(None) => self.push(format!("MOTHER: No evidence with id {}", evidence_id)),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            },
            Some(PendingAction::SkillNew { name, description }) => {
                match self.db.add_skill(&name, &description) {
                    Ok(id) => self.push(format!("MOTHER: Skill '{}' created (#{}).", name, id)),
                    Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
                }
            }
            Some(PendingAction::SkillAdd { name, text }) => match self.db.get_skill(&name) {
                Ok(Some(skill)) => {
                    let steps = self.db.list_skill_steps(skill.id).unwrap_or_default();
                    let next_no = steps.len() as i64 + 1;
                    match self.db.add_skill_step(skill.id, next_no, &text, None, None) {
                        Ok(id) => self.push(format!(
                            "MOTHER: Added step {} to '{}' (#{}).",
                            next_no, skill.name, id
                        )),
                        Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
                    }
                }
                Ok(None) => self.push(format!("MOTHER: No skill named '{}'.", name)),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            },
            Some(PendingAction::Suggestion { plans }) => {
                self.push("MOTHER: Plans acknowledged (no automatic execution).");
                for p in plans {
                    self.push(format!("  PLAN: {}", p));
                }
            }
            None => self.push("MOTHER: No pending proposal."),
        }
    }

    fn reject_pending(&mut self) {
        if let Some(PendingAction::Concept(p)) = self.pending.take() {
            let _ = self.db.record_confidence_event(&p.name, "reject_claim");
            self.push("MOTHER: Proposal rejected.");
            return;
        }
        if self.pending.take().is_some() {
            self.push("MOTHER: Proposal rejected.");
        } else {
            self.push("MOTHER: No pending proposal.");
        }
    }

    fn suggest_tags(&self, summary: &str) -> Option<Vec<String>> {
        let Ok(names) = self.db.list_concept_names(500) else {
            return None;
        };
        let lower = summary.to_lowercase();
        let mut found = HashSet::new();
        for name in names {
            if lower.contains(&name) {
                found.insert(name);
            }
        }
        if found.is_empty() {
            None
        } else {
            Some(found.into_iter().collect())
        }
    }

    fn derive_domain(source: &str) -> Option<String> {
        if let Some(idx) = source.find("://") {
            let rest = &source[idx + 3..];
            let parts: Vec<&str> = rest.split('/').collect();
            return parts.first().map(|s| s.to_string());
        }
        None
    }

    fn render_evidence_update(&mut self, ev: &Evidence) {
        self.push(format!(
            "MOTHER: Evidence #{} trust now {:.2} (concept {}).",
            ev.id, ev.trust, ev.concept_name
        ));
        if let Some(domain) = &ev.domain {
            self.push(format!("  Domain: {}", domain));
        }
    }

    fn gaps_report(&mut self) -> Option<Vec<String>> {
        let concepts = self.db.list_concepts(1000).unwrap_or_default();
        let rels = self.db.list_all_relations(10_000).unwrap_or_default();
        let evidence = concepts
            .iter()
            .filter_map(|c| {
                self.db
                    .list_evidence_for(&c.name, 1)
                    .ok()
                    .map(|v| (c.name.clone(), v.len()))
            })
            .collect::<Vec<_>>();

        let mut plans = Vec::new();

        // Concepts without evidence
        for (name, count) in evidence.iter() {
            if *count == 0 {
                plans.push(format!("search evidence for concept '{}'", name));
            }
        }

        // Concepts without relations
        let mut related: HashSet<String> = HashSet::new();
        for r in rels.iter() {
            related.insert(r.from.clone());
            related.insert(r.to.clone());
        }
        for c in concepts.iter() {
            if !related.contains(&c.name) {
                plans.push(format!("search relations for '{}'", c.name));
            }
        }

        // Low-trust evidence
        let mut low_trust_ids = Vec::new();
        for c in concepts.iter() {
            if let Ok(list) = self.db.list_evidence_for(&c.name, 50) {
                for ev in list {
                    if ev.trust < 0.3 {
                        low_trust_ids.push(ev.id);
                    }
                }
            }
        }
        for id in low_trust_ids {
            plans.push(format!("review evidence trust #{}", id));
        }

        // Low-confidence concepts
        for c in concepts.iter() {
            if c.confidence < 0.3 {
                plans.push(format!("search to reinforce '{}'", c.name));
            }
        }

        if plans.is_empty() { None } else { Some(plans) }
    }
}

impl Module for Dialog {
    fn render(&mut self, f: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(f.area());

        let text = self.history.join("\n");
        let dialog =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("DIALOG"));

        let prompt = if self.pending.is_some() {
            "[y]/[n] -> "
        } else {
            ""
        };
        let input = Paragraph::new(format!("{}{}", prompt, self.input.as_str()))
            .block(Block::default().borders(Borders::ALL).title("INPUT"));

        f.render_widget(dialog, layout[0]);
        f.render_widget(input, layout[1]);
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') if self.pending.is_some() => self.confirm_pending(),
            KeyCode::Char('n') if self.pending.is_some() => self.reject_pending(),
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Enter => {
                let line = self.input.clone();
                self.push(format!("YOU: {}", line));
                self.input.clear();
                self.handle_command(&line);
            }
            _ => {}
        }
    }
}
