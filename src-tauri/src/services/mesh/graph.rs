//! What a view of the Mesh draws (section 15, ADR 0022, decisions 2 to 5).
//!
//! The Global View starts with the top subnets as closed clusters (and one
//! for the elements without a subnet); opening a cluster shows its child
//! subnets and the elements whose primary subnet it is, inside its hull.
//! The Subnet View does the same below one subnet. Every connection
//! between elements that are not drawn is moved to what represents them:
//! the closest drawn subnet above, i.e. the closed cluster they are in.
//! Connections that end up between the same two nodes are summed up.
//! The Focus View shows one node and its direct connections, on request a
//! step further, without hulls.
//!
//! At most [`MAX_DRAWN_NODES`] nodes are drawn; parents come before their
//! children in the list (section 15, for React Flow).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::database::mesh::{ConnectionRow, ElementRow, MemberRow, PositionRow, SubnetNodeRow};

/// Section 15: "Höchstens 1.000 Knoten werden gleichzeitig gezeichnet."
pub const MAX_DRAWN_NODES: usize = 1000;
/// The id of the cluster of elements without a subnet ("Nicht zugeordnet").
pub const UNASSIGNED_ID: &str = "unassigned";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MeshView {
    Global,
    Subnet,
    Focus,
}

impl MeshView {
    pub fn as_str(self) -> &'static str {
        match self {
            MeshView::Global => "global",
            MeshView::Subnet => "subnet",
            MeshView::Focus => "focus",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MeshNodeKind {
    Document,
    Media,
    Subnet,
    /// The cluster of the elements without a subnet.
    Unassigned,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeshNode {
    pub id: String,
    pub kind: MeshNodeKind,
    /// `image`, `pdf` or `attachment` for a medium.
    pub media_kind: Option<String>,
    pub title: String,
    /// Colour token or `#rrggbb` of a subnet.
    pub color: Option<String>,
    pub archived: bool,
    /// The open hull the node is drawn in.
    pub parent_id: Option<String>,
    /// A subnet (or the unassigned cluster) drawn open, as a hull.
    pub open: bool,
    /// Elements in a subnet and below it, or without a subnet.
    pub count: u32,
    /// Subnets directly below a subnet.
    pub child_subnets: u32,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MeshEdgeKind {
    /// Drawn by the user; stored.
    Manual,
    /// An accepted suggestion; stored.
    Similarity,
    /// An internal link or a medium in a document; derived, directed.
    Reference,
    /// A further subnet of an element; derived.
    Membership,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeshEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: MeshEdgeKind,
    pub directed: bool,
    /// Connections summed up in this edge.
    pub count: u32,
    /// The stored connection, when the edge is exactly one of them.
    pub connection_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeshCrumb {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeshGraph {
    pub view: MeshView,
    pub scope_id: String,
    pub nodes: Vec<MeshNode>,
    pub edges: Vec<MeshEdge>,
    /// Nodes left out over [`MAX_DRAWN_NODES`].
    pub hidden: u32,
    /// From the top to the subnet of a Subnet View; the node of a Focus View.
    pub breadcrumb: Vec<MeshCrumb>,
}

/// What a view is asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQuery {
    pub view: MeshView,
    /// The subnet of a Subnet View, the node of a Focus View.
    pub scope_id: String,
    /// Subnets drawn open (and [`UNASSIGNED_ID`]).
    pub expanded: BTreeSet<String>,
    /// Focus View: 1 or 2 steps.
    pub depth: u8,
}

/// Everything a view is built from.
#[derive(Debug, Default)]
pub struct MeshData {
    pub elements: Vec<ElementRow>,
    pub subnets: Vec<SubnetNodeRow>,
    pub members: Vec<MemberRow>,
    pub references: Vec<(String, String)>,
    pub connections: Vec<ConnectionRow>,
}

/// The data, indexed.
struct Index<'a> {
    elements: HashMap<&'a str, &'a ElementRow>,
    subnets: HashMap<&'a str, &'a SubnetNodeRow>,
    /// Child subnets by parent (`""`: top level), by name.
    children: HashMap<&'a str, Vec<&'a str>>,
    /// Elements by their primary subnet, in element order.
    primary_members: HashMap<&'a str, Vec<&'a str>>,
    primary: HashMap<&'a str, &'a str>,
    unassigned: Vec<&'a str>,
    /// Elements in a subnet and below.
    counts: HashMap<&'a str, u32>,
}

impl<'a> Index<'a> {
    fn new(data: &'a MeshData) -> Self {
        let elements: HashMap<&str, &ElementRow> =
            data.elements.iter().map(|e| (e.id.as_str(), e)).collect();
        let subnets: HashMap<&str, &SubnetNodeRow> =
            data.subnets.iter().map(|s| (s.id.as_str(), s)).collect();
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for subnet in &data.subnets {
            children
                .entry(subnet.parent_id.as_deref().unwrap_or(""))
                .or_default()
                .push(subnet.id.as_str());
        }
        let mut primary = HashMap::new();
        for member in data.members.iter().filter(|m| m.is_primary) {
            if elements.contains_key(member.element_id.as_str()) {
                primary.insert(member.element_id.as_str(), member.subnet_id.as_str());
            }
        }
        let mut primary_members: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut unassigned = Vec::new();
        let mut counts: HashMap<&str, u32> = HashMap::new();
        for element in &data.elements {
            match primary.get(element.id.as_str()) {
                Some(&subnet) => {
                    primary_members
                        .entry(subnet)
                        .or_default()
                        .push(element.id.as_str());
                    let mut current = Some(subnet);
                    let mut steps = 0;
                    while let Some(id) = current {
                        *counts.entry(id).or_default() += 1;
                        current = subnets.get(id).and_then(|s| s.parent_id.as_deref());
                        steps += 1;
                        if steps > subnets.len() {
                            break;
                        }
                    }
                }
                None => unassigned.push(element.id.as_str()),
            }
        }
        Self {
            elements,
            subnets,
            children,
            primary_members,
            primary,
            unassigned,
            counts,
        }
    }

    fn parent(&self, subnet: &str) -> Option<&'a str> {
        self.subnets
            .get(subnet)
            .and_then(|s| s.parent_id.as_deref())
    }

    /// The subnet and the ones above it, from the subnet up.
    fn chain(&self, subnet: &'a str) -> Vec<&'a str> {
        let mut chain = vec![subnet];
        let mut current = self.parent(subnet);
        while let Some(id) = current {
            if chain.len() > self.subnets.len() {
                break;
            }
            chain.push(id);
            current = self.parent(id);
        }
        chain
    }

    fn is_below(&self, subnet: &str, ancestor: &str) -> bool {
        self.chain_of(subnet).contains(&ancestor)
    }

    fn chain_of(&self, subnet: &str) -> Vec<&'a str> {
        match self.subnets.get_key_value(subnet) {
            Some((&key, _)) => self.chain(key),
            None => Vec::new(),
        }
    }

    fn subnet_node(&self, id: &str, parent: Option<&str>, open: bool) -> MeshNode {
        let subnet = self.subnets.get(id);
        MeshNode {
            id: id.to_owned(),
            kind: MeshNodeKind::Subnet,
            media_kind: None,
            title: subnet.map(|s| s.name.clone()).unwrap_or_default(),
            color: subnet.map(|s| s.color.clone()),
            archived: false,
            parent_id: parent.map(str::to_owned),
            open,
            count: self.counts.get(id).copied().unwrap_or(0),
            child_subnets: u32::try_from(self.children.get(id).map_or(0, Vec::len))
                .unwrap_or(u32::MAX),
            x: None,
            y: None,
            pinned: false,
        }
    }

    fn element_node(&self, id: &str, parent: Option<&str>) -> MeshNode {
        let element = self.elements.get(id);
        let is_document = element.is_none_or(|e| e.is_document);
        MeshNode {
            id: id.to_owned(),
            kind: if is_document {
                MeshNodeKind::Document
            } else {
                MeshNodeKind::Media
            },
            media_kind: element.filter(|e| !e.is_document).map(|e| e.kind.clone()),
            title: element.map(|e| e.title.clone()).unwrap_or_default(),
            color: None,
            archived: element.is_some_and(|e| e.archived),
            parent_id: parent.map(str::to_owned),
            open: false,
            count: 0,
            child_subnets: 0,
            x: None,
            y: None,
            pinned: false,
        }
    }

    fn unassigned_node(&self, open: bool) -> MeshNode {
        MeshNode {
            id: UNASSIGNED_ID.to_owned(),
            kind: MeshNodeKind::Unassigned,
            media_kind: None,
            title: String::new(),
            color: Some("grey".to_owned()),
            archived: false,
            parent_id: None,
            open,
            count: u32::try_from(self.unassigned.len()).unwrap_or(u32::MAX),
            child_subnets: 0,
            x: None,
            y: None,
            pinned: false,
        }
    }
}

/// Nodes in drawing order, with a limit.
#[derive(Default)]
struct Drawn {
    nodes: Vec<MeshNode>,
    ids: HashSet<String>,
    hidden: u32,
}

impl Drawn {
    fn push(&mut self, node: MeshNode) -> bool {
        if self.nodes.len() >= MAX_DRAWN_NODES {
            self.hidden += 1;
            return false;
        }
        self.ids.insert(node.id.clone());
        self.nodes.push(node);
        true
    }

    fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }
}

/// Builds the view `query` asks for. `positions` are the stored ones of
/// the view's layout.
pub fn build(data: &MeshData, query: &GraphQuery, positions: &[PositionRow]) -> MeshGraph {
    let index = Index::new(data);
    let (mut drawn, breadcrumb) = match query.view {
        MeshView::Global => (tree(&index, None, &query.expanded), Vec::new()),
        MeshView::Subnet => {
            let breadcrumb = index
                .chain_of(&query.scope_id)
                .into_iter()
                .rev()
                .map(|id| MeshCrumb {
                    id: id.to_owned(),
                    title: index
                        .subnets
                        .get(id)
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                })
                .collect();
            (
                tree(&index, Some(&query.scope_id), &query.expanded),
                breadcrumb,
            )
        }
        MeshView::Focus => {
            let title = index
                .elements
                .get(query.scope_id.as_str())
                .map(|e| e.title.clone())
                .or_else(|| {
                    index
                        .subnets
                        .get(query.scope_id.as_str())
                        .map(|s| s.name.clone())
                })
                .unwrap_or_default();
            (
                focus(&index, data, &query.scope_id, query.depth),
                vec![MeshCrumb {
                    id: query.scope_id.clone(),
                    title,
                }],
            )
        }
    };
    let edges = match query.view {
        MeshView::Focus => focus_edges(data, &drawn),
        _ => tree_edges(&index, data, &drawn, query),
    };
    let by_node: HashMap<&str, &PositionRow> =
        positions.iter().map(|p| (p.node_id.as_str(), p)).collect();
    for node in &mut drawn.nodes {
        if let Some(position) = by_node.get(node.id.as_str()) {
            node.x = Some(position.x);
            node.y = Some(position.y);
            node.pinned = position.pinned;
        }
    }
    MeshGraph {
        view: query.view,
        scope_id: query.scope_id.clone(),
        nodes: drawn.nodes,
        edges,
        hidden: drawn.hidden,
        breadcrumb,
    }
}

/// Global View (`scope` `None`) or Subnet View: open subnets breadth first,
/// so every hull comes before what it holds.
fn tree(index: &Index<'_>, scope: Option<&str>, expanded: &BTreeSet<String>) -> Drawn {
    let mut drawn = Drawn::default();
    let mut queue: VecDeque<String> = VecDeque::new();
    match scope {
        None => {
            for &root in index.children.get("").into_iter().flatten() {
                let open = expanded.contains(root);
                if drawn.push(index.subnet_node(root, None, open)) && open {
                    queue.push_back(root.to_owned());
                }
            }
            if !index.unassigned.is_empty() {
                let open = expanded.contains(UNASSIGNED_ID);
                if drawn.push(index.unassigned_node(open)) && open {
                    queue.push_back(UNASSIGNED_ID.to_owned());
                }
            }
        }
        Some(subnet) => {
            if index.subnets.contains_key(subnet)
                && drawn.push(index.subnet_node(subnet, None, true))
            {
                queue.push_back(subnet.to_owned());
            }
        }
    }
    while let Some(hull) = queue.pop_front() {
        if hull == UNASSIGNED_ID {
            for &element in &index.unassigned {
                drawn.push(index.element_node(element, Some(UNASSIGNED_ID)));
            }
            continue;
        }
        for &child in index.children.get(hull.as_str()).into_iter().flatten() {
            let open = expanded.contains(child);
            if drawn.push(index.subnet_node(child, Some(&hull), open)) && open {
                queue.push_back(child.to_owned());
            }
        }
        for &element in index
            .primary_members
            .get(hull.as_str())
            .into_iter()
            .flatten()
        {
            drawn.push(index.element_node(element, Some(&hull)));
        }
    }
    drawn
}

/// The drawn node that stands for `id` (an element or a subnet): itself,
/// or the closest drawn subnet above it, or the unassigned cluster.
fn representative(index: &Index<'_>, drawn: &Drawn, id: &str) -> Option<String> {
    if drawn.contains(id) {
        return Some(id.to_owned());
    }
    if index.elements.contains_key(id) {
        return match index.primary.get(id) {
            Some(&subnet) => index
                .chain(subnet)
                .into_iter()
                .find(|s| drawn.contains(s))
                .map(str::to_owned),
            None => drawn
                .contains(UNASSIGNED_ID)
                .then(|| UNASSIGNED_ID.to_owned()),
        };
    }
    index
        .chain_of(id)
        .into_iter()
        .find(|s| drawn.contains(s))
        .map(str::to_owned)
}

/// Summed-up edges between drawn nodes.
#[derive(Default)]
struct Edges {
    by_key: BTreeMap<(MeshEdgeKind, String, String), (u32, Option<String>)>,
}

impl Edges {
    fn add(
        &mut self,
        kind: MeshEdgeKind,
        directed: bool,
        from: String,
        to: String,
        connection: Option<String>,
    ) {
        if from == to {
            return;
        }
        let (a, b) = if directed || from < to {
            (from, to)
        } else {
            (to, from)
        };
        let entry = self.by_key.entry((kind, a, b)).or_insert((0, None));
        entry.0 += 1;
        entry.1 = if entry.0 == 1 { connection } else { None };
    }

    fn into_edges(self) -> Vec<MeshEdge> {
        self.by_key
            .into_iter()
            .map(
                |((kind, source, target), (count, connection_id))| MeshEdge {
                    id: format!("{}:{source}:{target}", kind_name(kind)),
                    directed: kind == MeshEdgeKind::Reference,
                    source,
                    target,
                    kind,
                    count,
                    connection_id,
                },
            )
            .collect()
    }
}

fn kind_name(kind: MeshEdgeKind) -> &'static str {
    match kind {
        MeshEdgeKind::Manual => "manual",
        MeshEdgeKind::Similarity => "similarity",
        MeshEdgeKind::Reference => "reference",
        MeshEdgeKind::Membership => "membership",
    }
}

fn connection_kind(kind: &str) -> MeshEdgeKind {
    if kind == "similarity" {
        MeshEdgeKind::Similarity
    } else {
        MeshEdgeKind::Manual
    }
}

fn tree_edges(
    index: &Index<'_>,
    data: &MeshData,
    drawn: &Drawn,
    query: &GraphQuery,
) -> Vec<MeshEdge> {
    let scope = (query.view == MeshView::Subnet).then_some(query.scope_id.as_str());
    // In a Subnet View only what lies in the subnet counts.
    let inside = |id: &str| -> bool {
        let Some(scope) = scope else { return true };
        if let Some(&subnet) = index.primary.get(id) {
            return index.is_below(subnet, scope);
        }
        index.subnets.contains_key(id) && index.is_below(id, scope)
    };
    let mut edges = Edges::default();
    for (from, to) in &data.references {
        if !inside(from) || !inside(to) {
            continue;
        }
        if let (Some(a), Some(b)) = (
            representative(index, drawn, from),
            representative(index, drawn, to),
        ) {
            edges.add(MeshEdgeKind::Reference, true, a, b, None);
        }
    }
    for connection in &data.connections {
        if !inside(&connection.node_a) || !inside(&connection.node_b) {
            continue;
        }
        if let (Some(a), Some(b)) = (
            representative(index, drawn, &connection.node_a),
            representative(index, drawn, &connection.node_b),
        ) {
            let exact = a == connection.node_a && b == connection.node_b;
            edges.add(
                connection_kind(&connection.kind),
                false,
                a,
                b,
                exact.then(|| connection.id.clone()),
            );
        }
    }
    // Further subnets of an element: an edge to that subnet, unless the
    // element already sits inside it.
    let hulls: HashMap<&str, Option<&str>> = drawn
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.parent_id.as_deref()))
        .collect();
    let inside_hull = |node: &str, hull: &str| -> bool {
        let mut current = hulls.get(node).copied().flatten();
        let mut steps = 0;
        while let Some(parent) = current {
            if parent == hull {
                return true;
            }
            steps += 1;
            if steps > MAX_DRAWN_NODES {
                break;
            }
            current = hulls.get(parent).copied().flatten();
        }
        false
    };
    for member in data.members.iter().filter(|m| !m.is_primary) {
        if !index.elements.contains_key(member.element_id.as_str())
            || !inside(&member.element_id)
            || !inside(&member.subnet_id)
        {
            continue;
        }
        if let (Some(a), Some(b)) = (
            representative(index, drawn, &member.element_id),
            representative(index, drawn, &member.subnet_id),
        ) && !inside_hull(&a, &b)
        {
            edges.add(MeshEdgeKind::Membership, false, a, b, None);
        }
    }
    edges.into_edges()
}

/// Focus View: the node, its direct neighbours, on request theirs.
fn focus(index: &Index<'_>, data: &MeshData, center: &str, depth: u8) -> Drawn {
    let mut neighbours: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut link = |a: &'_ str, b: &'_ str| {
        let (Some(a), Some(b)) = (intern(index, a), intern(index, b)) else {
            return;
        };
        neighbours.entry(a).or_default().push(b);
        neighbours.entry(b).or_default().push(a);
    };
    for (from, to) in &data.references {
        link(from, to);
    }
    for connection in &data.connections {
        link(&connection.node_a, &connection.node_b);
    }
    for member in &data.members {
        link(&member.element_id, &member.subnet_id);
    }
    for subnet in &data.subnets {
        if let Some(parent) = &subnet.parent_id {
            link(&subnet.id, parent);
        }
    }
    let mut drawn = Drawn::default();
    let Some(center) = intern(index, center) else {
        return drawn;
    };
    let mut seen: HashSet<&str> = HashSet::new();
    let mut level = vec![center];
    seen.insert(center);
    push_any(index, &mut drawn, center);
    for _ in 0..depth.clamp(1, 2) {
        let mut next = Vec::new();
        for &node in &level {
            let mut around: Vec<&str> = neighbours.get(node).cloned().unwrap_or_default();
            around.sort_unstable();
            around.dedup();
            for other in around {
                if seen.insert(other) {
                    push_any(index, &mut drawn, other);
                    next.push(other);
                }
            }
        }
        level = next;
    }
    drawn
}

/// The id as it is stored in the index (elements and subnets only).
fn intern<'a>(index: &Index<'a>, id: &str) -> Option<&'a str> {
    index
        .elements
        .get_key_value(id)
        .map(|(&key, _)| key)
        .or_else(|| index.subnets.get_key_value(id).map(|(&key, _)| key))
}

fn push_any(index: &Index<'_>, drawn: &mut Drawn, id: &str) {
    if index.subnets.contains_key(id) {
        drawn.push(index.subnet_node(id, None, false));
    } else {
        drawn.push(index.element_node(id, None));
    }
}

fn focus_edges(data: &MeshData, drawn: &Drawn) -> Vec<MeshEdge> {
    let mut edges = Edges::default();
    let both = |a: &str, b: &str| drawn.contains(a) && drawn.contains(b);
    for (from, to) in &data.references {
        if both(from, to) {
            edges.add(
                MeshEdgeKind::Reference,
                true,
                from.clone(),
                to.clone(),
                None,
            );
        }
    }
    for connection in &data.connections {
        if both(&connection.node_a, &connection.node_b) {
            edges.add(
                connection_kind(&connection.kind),
                false,
                connection.node_a.clone(),
                connection.node_b.clone(),
                Some(connection.id.clone()),
            );
        }
    }
    for member in &data.members {
        if both(&member.element_id, &member.subnet_id) {
            edges.add(
                MeshEdgeKind::Membership,
                false,
                member.element_id.clone(),
                member.subnet_id.clone(),
                None,
            );
        }
    }
    for subnet in &data.subnets {
        if let Some(parent) = &subnet.parent_id
            && both(&subnet.id, parent)
        {
            edges.add(
                MeshEdgeKind::Membership,
                false,
                subnet.id.clone(),
                parent.clone(),
                None,
            );
        }
    }
    edges.into_edges()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(id: &str, title: &str) -> ElementRow {
        ElementRow {
            id: id.to_owned(),
            kind: "document".to_owned(),
            title: title.to_owned(),
            archived: false,
            is_document: true,
            import_folder: None,
        }
    }

    fn subnet(id: &str, parent: Option<&str>) -> SubnetNodeRow {
        SubnetNodeRow {
            id: id.to_owned(),
            name: id.to_uppercase(),
            color: "blue".to_owned(),
            parent_id: parent.map(str::to_owned),
        }
    }

    fn member(element: &str, subnet: &str, primary: bool) -> MemberRow {
        MemberRow {
            element_id: element.to_owned(),
            subnet_id: subnet.to_owned(),
            is_primary: primary,
        }
    }

    /// Subnets a > a1, b; documents d1 in a1, d2 in a (and b), d3 in b, d4 without.
    fn data() -> MeshData {
        MeshData {
            elements: vec![
                element("d1", "Eins"),
                element("d2", "Zwei"),
                element("d3", "Drei"),
                element("d4", "Vier"),
            ],
            subnets: vec![
                subnet("a", None),
                subnet("a1", Some("a")),
                subnet("b", None),
            ],
            members: vec![
                member("d1", "a1", true),
                member("d2", "a", true),
                member("d2", "b", false),
                member("d3", "b", true),
            ],
            references: vec![
                ("d1".to_owned(), "d3".to_owned()),
                ("d2".to_owned(), "d3".to_owned()),
                ("d4".to_owned(), "d1".to_owned()),
            ],
            connections: vec![ConnectionRow {
                id: "c1".to_owned(),
                node_a: "d1".to_owned(),
                node_b: "d2".to_owned(),
                kind: "manual".to_owned(),
                note: None,
                created_at: String::new(),
            }],
        }
    }

    fn query(view: MeshView, scope: &str, expanded: &[&str]) -> GraphQuery {
        GraphQuery {
            view,
            scope_id: scope.to_owned(),
            expanded: expanded.iter().map(|s| (*s).to_owned()).collect(),
            depth: 1,
        }
    }

    fn ids(graph: &MeshGraph) -> Vec<&str> {
        graph.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    fn edge(graph: &MeshGraph, kind: MeshEdgeKind) -> Vec<(String, String, u32)> {
        graph
            .edges
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| (e.source.clone(), e.target.clone(), e.count))
            .collect()
    }

    #[test]
    fn the_global_view_starts_with_closed_clusters_and_sums_up_their_edges() {
        let graph = build(&data(), &query(MeshView::Global, "", &[]), &[]);
        assert_eq!(ids(&graph), vec!["a", "b", UNASSIGNED_ID]);
        let a = &graph.nodes[0];
        assert_eq!((a.count, a.child_subnets, a.open), (2, 1, false));
        // d1→d3 and d2→d3 between a and b; d4→d1 from the unassigned cluster.
        assert_eq!(
            edge(&graph, MeshEdgeKind::Reference),
            vec![
                ("a".to_owned(), "b".to_owned(), 2),
                (UNASSIGNED_ID.to_owned(), "a".to_owned(), 1)
            ]
        );
        // d1–d2 lie both in a: no edge. d2 is also in b: an edge a–b.
        assert!(edge(&graph, MeshEdgeKind::Manual).is_empty());
        assert_eq!(
            edge(&graph, MeshEdgeKind::Membership),
            vec![("a".to_owned(), "b".to_owned(), 1)]
        );
    }

    #[test]
    fn opening_a_cluster_draws_its_content_inside_its_hull_after_it() {
        let graph = build(&data(), &query(MeshView::Global, "", &["a", "a1"]), &[]);
        assert_eq!(ids(&graph), vec!["a", "b", UNASSIGNED_ID, "a1", "d2", "d1"]);
        let d1 = graph.nodes.iter().find(|n| n.id == "d1").unwrap();
        assert_eq!(d1.parent_id.as_deref(), Some("a1"));
        // The manual connection is drawn as itself and can be removed.
        let manual: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == MeshEdgeKind::Manual)
            .collect();
        assert_eq!(manual.len(), 1);
        assert_eq!(manual[0].connection_id.as_deref(), Some("c1"));
        assert_eq!(
            edge(&graph, MeshEdgeKind::Reference),
            vec![
                ("d1".to_owned(), "b".to_owned(), 1),
                ("d2".to_owned(), "b".to_owned(), 1),
                (UNASSIGNED_ID.to_owned(), "d1".to_owned(), 1)
            ]
        );
        assert_eq!(
            edge(&graph, MeshEdgeKind::Membership),
            vec![("b".to_owned(), "d2".to_owned(), 1)]
        );
    }

    #[test]
    fn the_subnet_view_shows_only_what_lies_in_the_subnet() {
        let graph = build(&data(), &query(MeshView::Subnet, "a", &["a1"]), &[]);
        assert_eq!(ids(&graph), vec!["a", "a1", "d2", "d1"]);
        assert_eq!(
            graph.breadcrumb,
            vec![MeshCrumb {
                id: "a".to_owned(),
                title: "A".to_owned()
            }]
        );
        assert!(edge(&graph, MeshEdgeKind::Reference).is_empty());
        assert_eq!(edge(&graph, MeshEdgeKind::Manual).len(), 1);
    }

    #[test]
    fn the_focus_view_shows_the_neighbours_and_on_request_theirs() {
        let graph = build(&data(), &query(MeshView::Focus, "d3", &[]), &[]);
        let mut found = ids(&graph);
        found.sort_unstable();
        assert_eq!(found, vec!["b", "d1", "d2", "d3"]);
        let mut wide = query(MeshView::Focus, "d3", &[]);
        wide.depth = 2;
        let graph = build(&data(), &wide, &[]);
        // d1 brings d4 and a1, d2 brings a.
        assert_eq!(graph.nodes.len(), 7);
        assert!(graph.nodes.iter().all(|n| n.parent_id.is_none()));
    }

    #[test]
    fn at_most_a_thousand_nodes_are_drawn_parents_first() {
        let mut data = data();
        for n in 0..1500 {
            let id = format!("x{n:04}");
            data.elements.push(element(&id, &id));
            data.members.push(member(&id, "b", true));
        }
        let graph = build(&data, &query(MeshView::Global, "", &["b"]), &[]);
        assert_eq!(graph.nodes.len(), MAX_DRAWN_NODES);
        assert_eq!(graph.hidden as usize, 3 + 1501 - MAX_DRAWN_NODES);
        let position: HashMap<&str, usize> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        for node in &graph.nodes {
            if let Some(parent) = &node.parent_id {
                assert!(position[parent.as_str()] < position[node.id.as_str()]);
            }
        }
    }

    #[test]
    fn stored_positions_are_put_on_the_nodes() {
        let positions = [PositionRow {
            node_id: "b".to_owned(),
            x: 10.0,
            y: -5.0,
            pinned: true,
        }];
        let graph = build(&data(), &query(MeshView::Global, "", &[]), &positions);
        let b = graph.nodes.iter().find(|n| n.id == "b").unwrap();
        assert_eq!((b.x, b.y, b.pinned), (Some(10.0), Some(-5.0), true));
    }
}
