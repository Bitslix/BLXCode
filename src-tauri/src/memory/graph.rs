use std::collections::{BTreeSet, HashMap};

use super::frontmatter::strip_frontmatter;
use super::paths::{category_hub_node_id, category_hub_path, graph_category_for, node_id};
use super::types::{GraphData, GraphEdge, GraphNode, MemoryScope};
use super::wikilinks::{parse_links_and_tags, resolve_link_target};

pub struct ScopeNote {
    pub scope: MemoryScope,
    pub path: String,
    pub body: String,
    pub label: String,
}

pub struct CategoryHubInput {
    pub category: String,
    pub label: String,
    pub scopes: Vec<MemoryScope>,
}

pub fn build_graph(notes: Vec<ScopeNote>, category_hubs: Vec<CategoryHubInput>) -> GraphData {
    let mut scope_paths: HashMap<MemoryScope, Vec<String>> = HashMap::new();
    for n in &notes {
        scope_paths
            .entry(n.scope.clone())
            .or_default()
            .push(n.path.clone());
    }

    let mut nodes: Vec<GraphNode> = notes
        .iter()
        .map(|n| {
            let body = strip_frontmatter(&n.body);
            let pr = parse_links_and_tags(&body);
            GraphNode {
                id: node_id(&n.scope, &n.path),
                scope: n.scope.clone(),
                path: n.path.clone(),
                label: n.label.clone(),
                tags: pr.tags,
                orphan: true,
                category: graph_category_for(&n.path),
                is_category_hub: None,
                hub_scopes: None,
            }
        })
        .collect();

    let mut hub_ids: BTreeSet<String> = BTreeSet::new();
    for hub in &category_hubs {
        let id = category_hub_node_id(&hub.category);
        if hub_ids.contains(&id) {
            continue;
        }
        hub_ids.insert(id.clone());
        let scope = if hub.scopes.contains(&MemoryScope::Workspace) {
            MemoryScope::Workspace
        } else {
            hub.scopes
                .first()
                .cloned()
                .unwrap_or(MemoryScope::Workspace)
        };
        nodes.push(GraphNode {
            id,
            scope,
            path: category_hub_path(&hub.category),
            label: hub.label.clone(),
            tags: Vec::new(),
            orphan: false,
            category: hub.category.clone(),
            is_category_hub: Some(true),
            hub_scopes: Some(hub.scopes.clone()),
        });
    }

    let node_set: BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let hub_id_set: BTreeSet<String> = nodes
        .iter()
        .filter(|n| n.is_category_hub.unwrap_or(false))
        .map(|n| n.id.clone())
        .collect();

    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut edge_keys: BTreeSet<String> = BTreeSet::new();

    for note in &notes {
        let body = strip_frontmatter(&note.body);
        let pr = parse_links_and_tags(&body);
        let source_id = node_id(&note.scope, &note.path);
        for link in &pr.links {
            let Some((target_scope, target_path)) =
                resolve_link_target(&note.scope, link, &scope_paths)
            else {
                continue;
            };
            let target_id = node_id(&target_scope, &target_path);
            if !node_set.contains(&target_id) || hub_id_set.contains(&target_id) {
                continue;
            }
            let key = format!("{source_id}->{target_id}");
            if edge_keys.contains(&key) {
                continue;
            }
            edge_keys.insert(key);
            let cross_scope = note.scope != target_scope;
            edges.push(GraphEdge {
                source: source_id.clone(),
                target: target_id,
                cross_scope,
                label: link.alias.clone(),
            });
        }
    }

    // Note → hub edges
    for note in &notes {
        let note_id = node_id(&note.scope, &note.path);
        let cat = graph_category_for(&note.path);
        if cat == "memory" {
            continue;
        }
        let hub_id = category_hub_node_id(&cat);
        if !hub_id_set.contains(&hub_id) {
            continue;
        }
        let key = format!("{note_id}->{hub_id}");
        if edge_keys.contains(&key) {
            continue;
        }
        edge_keys.insert(key);
        edges.push(GraphEdge {
            source: note_id,
            target: hub_id,
            cross_scope: false,
            label: None,
        });
    }

    // Mark orphan nodes
    let mut connected: BTreeSet<String> = BTreeSet::new();
    for e in &edges {
        connected.insert(e.source.clone());
        connected.insert(e.target.clone());
    }
    for n in nodes.iter_mut() {
        if n.is_category_hub.unwrap_or(false) {
            n.orphan = false;
            continue;
        }
        n.orphan = !connected.contains(&n.id);
    }

    GraphData { nodes, edges }
}
