use std::collections::{HashMap, HashSet};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use super::Module;
use crate::db::{Concept, Database, Relation};

enum GraphMode {
    Relations,
    Map,
}

struct ClusterInfo {
    nodes: Vec<String>,
    top: Vec<String>,
}

pub struct Graph {
    db: Database,
    concepts: Vec<Concept>,
    selected: usize,
    status: String,
    mode: GraphMode,
    clusters: Vec<ClusterInfo>,
    cluster_selected: usize,
    cluster_concept_selected: usize,
}

impl Graph {
    pub fn new(db: Database) -> Self {
        let mut g = Self {
            db,
            concepts: Vec::new(),
            selected: 0,
            status: "GRAPH READY. Use ↑/↓. [m] map view. [r] refresh. [Ctrl+C] CONSOLE [Ctrl+D] DIALOG [Ctrl+Q] QUIT".to_string(),
            mode: GraphMode::Relations,
            clusters: Vec::new(),
            cluster_selected: 0,
            cluster_concept_selected: 0,
        };
        g.refresh();
        g
    }

    fn refresh(&mut self) {
        match self.db.list_concepts(500) {
            Ok(list) => {
                self.concepts = list;
                if self.selected >= self.concepts.len() {
                    self.selected = self.concepts.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("DB error: {}", e),
        }

        match self.db.list_all_relations(10_000) {
            Ok(rels) => self.clusters = compute_clusters(&self.concepts, &rels),
            Err(e) => self.status = format!("DB error: {}", e),
        }
    }

    fn selected_name(&self) -> Option<&str> {
        self.concepts.get(self.selected).map(|c| c.name.as_str())
    }

    fn select_concept_by_name(&mut self, name: &str) {
        if let Some(idx) = self.concepts.iter().position(|c| c.name == name) {
            self.selected = idx;
            self.mode = GraphMode::Relations;
        }
    }
}

impl Module for Graph {
    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(f.area());

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        // Header / status
        let header = Paragraph::new(self.status.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("MOTHER / GRAPH"),
        );
        f.render_widget(header, chunks[0]);

        match self.mode {
            GraphMode::Relations => {
                // Left: concept list
                let items: Vec<ListItem> = self
                    .concepts
                    .iter()
                    .enumerate()
                    .map(|(i, concept)| {
                        let label = format!("{} ({:.2})", concept.name, concept.confidence);
                        if i == self.selected {
                            ListItem::new(format!("> {}", label))
                        } else {
                            ListItem::new(format!("  {}", label))
                        }
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title("CONCEPTS"));

                f.render_widget(list, body[0]);

                // Right: details for selected concept
                let right_text = if let Some(name) = self.selected_name() {
                    render_concept_view(name, &self.db)
                } else {
                    "No concepts found.\nGo to DIALOG and add one using:\nlearn <concept> is <definition>\n".to_string()
                };

                let rel_view = Paragraph::new(right_text)
                    .block(Block::default().borders(Borders::ALL).title("DETAILS"));

                f.render_widget(rel_view, body[1]);
            }
            GraphMode::Map => {
                let clusters: Vec<ListItem> = self
                    .clusters
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let label = format!("Cluster {} ({} concepts)", i + 1, c.nodes.len());
                        if i == self.cluster_selected {
                            ListItem::new(format!("> {}", label))
                        } else {
                            ListItem::new(format!("  {}", label))
                        }
                    })
                    .collect();

                let list = List::new(clusters).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("CLUSTERS (↑/↓)"),
                );
                f.render_widget(list, body[0]);

                let right_text = if let Some(cluster) = self.clusters.get(self.cluster_selected) {
                    let mut out = String::new();
                    out.push_str("MAP VIEW: connected concepts\n");
                    if !cluster.top.is_empty() {
                        out.push_str("Top nodes:\n");
                        for n in cluster.top.iter() {
                            out.push_str(&format!("  - {}\n", n));
                        }
                    }
                    out.push_str("\nConcepts (←/→ to choose, Enter to open):\n");
                    if cluster.nodes.is_empty() {
                        out.push_str("  (empty)\n");
                    } else {
                        for (i, n) in cluster.nodes.iter().enumerate() {
                            if i == self.cluster_concept_selected {
                                out.push_str(&format!("  > {}\n", n));
                            } else {
                                out.push_str(&format!("    {}\n", n));
                            }
                        }
                    }
                    out
                } else {
                    "No clusters calculated.".to_string()
                };

                let rel_view = Paragraph::new(right_text)
                    .block(Block::default().borders(Borders::ALL).title("MAP DETAILS"));

                f.render_widget(rel_view, body[1]);
            }
        }
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match self.mode {
            GraphMode::Relations => match key.code {
                KeyCode::Up => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.selected + 1 < self.concepts.len() {
                        self.selected += 1;
                    }
                }
                KeyCode::Char('r') => self.refresh(),
                KeyCode::Char('m') => {
                    self.mode = GraphMode::Map;
                    self.cluster_selected = 0;
                    self.cluster_concept_selected = 0;
                }
                _ => {}
            },
            GraphMode::Map => match key.code {
                KeyCode::Up => {
                    if self.cluster_selected > 0 {
                        self.cluster_selected -= 1;
                        self.cluster_concept_selected = 0;
                    }
                }
                KeyCode::Down => {
                    if self.cluster_selected + 1 < self.clusters.len() {
                        self.cluster_selected += 1;
                        self.cluster_concept_selected = 0;
                    }
                }
                KeyCode::Left => {
                    if self.cluster_concept_selected > 0 {
                        self.cluster_concept_selected -= 1;
                    }
                }
                KeyCode::Right => {
                    if let Some(cluster) = self.clusters.get(self.cluster_selected) {
                        if self.cluster_concept_selected + 1 < cluster.nodes.len() {
                            self.cluster_concept_selected += 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    let target = self
                        .clusters
                        .get(self.cluster_selected)
                        .and_then(|c| c.nodes.get(self.cluster_concept_selected))
                        .cloned();
                    if let Some(name) = target {
                        self.select_concept_by_name(&name);
                    }
                }
                KeyCode::Char('m') => self.mode = GraphMode::Relations,
                KeyCode::Char('r') => self.refresh(),
                _ => {}
            },
        }
    }
}

fn render_concept_view(name: &str, db: &Database) -> String {
    let mut out = String::new();
    let concept = db.get_concept(name).ok().flatten();
    if let Some(c) = concept {
        out.push_str(&format!("FOCUS: {} (conf {:.2})\n", c.name, c.confidence));
        out.push_str(&format!("Def: {}\n", c.definition));
    }

    if let Ok(rels) = db.list_relations_for(name, 200) {
        out.push_str("\nRelations:\n");
        if rels.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for r in rels.iter().filter(|r| r.from == name) {
                out.push_str(&format!("  {} --{}--> {}\n", r.from, r.relation_type, r.to));
            }
            for r in rels.iter().filter(|r| r.to == name) {
                out.push_str(&format!("  {} --{}--> {}\n", r.from, r.relation_type, r.to));
            }
        }
    }

    if let Ok(ev) = db.list_evidence_for(name, 10) {
        out.push_str("\nEvidence (trust):\n");
        if ev.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for e in ev {
                let src = e
                    .domain
                    .clone()
                    .or(e.source.clone())
                    .unwrap_or_else(|| "-".to_string());
                out.push_str(&format!(
                    "  #{} {:.2} [{}] {}\n",
                    e.id, e.trust, src, e.content
                ));
            }
        }
    }

    if let Ok(episodes) = db.list_episodes_for_concept(name, 5) {
        out.push_str("\nEpisodes (latest):\n");
        if episodes.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for e in episodes {
                out.push_str(&format!(
                    "  [{}] #{} {}\n    {}\n",
                    e.outcome, e.id, e.captured_at, e.summary
                ));
                if let Ok(tags) = db.list_episode_tags(e.id) {
                    if !tags.is_empty() {
                        out.push_str(&format!("    tags: {}\n", tags.join(", ")));
                    }
                }
            }
        }
    }

    out
}

fn compute_clusters(concepts: &[Concept], rels: &[Relation]) -> Vec<ClusterInfo> {
    let mut adjacency: HashMap<String, HashSet<String>> = HashMap::new();
    for c in concepts {
        adjacency.entry(c.name.clone()).or_default();
    }
    for r in rels {
        adjacency
            .entry(r.from.clone())
            .or_default()
            .insert(r.to.clone());
        adjacency
            .entry(r.to.clone())
            .or_default()
            .insert(r.from.clone());
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut clusters = Vec::new();

    for node in adjacency.keys() {
        if visited.contains(node) {
            continue;
        }
        let mut stack = vec![node.clone()];
        let mut nodes = Vec::new();
        visited.insert(node.clone());
        while let Some(n) = stack.pop() {
            nodes.push(n.clone());
            if let Some(nei) = adjacency.get(&n) {
                for neigh in nei {
                    if visited.insert(neigh.clone()) {
                        stack.push(neigh.clone());
                    }
                }
            }
        }
        nodes.sort();
        let top = top_nodes(&nodes, &adjacency);
        clusters.push(ClusterInfo { nodes, top });
    }

    clusters.sort_by_key(|c| -(c.nodes.len() as isize));
    clusters
}

fn top_nodes(nodes: &[String], adjacency: &HashMap<String, HashSet<String>>) -> Vec<String> {
    let mut scored: Vec<(usize, String)> = nodes
        .iter()
        .map(|n| (adjacency.get(n).map(|s| s.len()).unwrap_or(0), n.clone()))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.iter().take(3).map(|(_, n)| n.clone()).collect()
}
