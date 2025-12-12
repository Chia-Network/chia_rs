use crate::LeftChildFirstIterator;
use crate::merkle::error::Error;
use crate::{InternalNode, LeafNode, MerkleBlob, Node, TreeIndex};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use url::Url;

pub struct DotLines {
    pub nodes: Vec<String>,
    pub connections: Vec<String>,
    pub pair_boxes: Vec<String>,
    pub traversal: Vec<String>,
    pub note: String,
    pub last_traversed_index: Option<TreeIndex>,
}

impl Default for DotLines {
    fn default() -> Self {
        Self::new()
    }
}

impl DotLines {
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            connections: vec![],
            pair_boxes: vec![],
            traversal: vec![],
            note: String::new(),
            last_traversed_index: None,
        }
    }

    pub fn push(&mut self, mut other: DotLines) {
        self.nodes.append(&mut other.nodes);
        self.connections.append(&mut other.connections);
        self.pair_boxes.append(&mut other.pair_boxes);
        self.traversal.append(&mut other.traversal);
    }

    pub fn push_traversal(&mut self, index: TreeIndex) {
        if let Some(last_index) = self.last_traversed_index {
            self.traversal.push(format!(
                r#"node_{last_index} -> node_{index} [constraint=false; color="red"]"#
            ));
        }
        self.last_traversed_index = Some(index);
    }

    pub fn dump(&mut self) -> String {
        // TODO: consuming itself, secretly
        let note = &self.note;
        let mut result = vec![];
        if !note.is_empty() {
            result.push(format!("# {note}"));
            result.push(String::new());
        }
        result.push("digraph {".to_string());
        result.append(&mut self.nodes);
        result.append(&mut self.connections);
        result.append(&mut self.pair_boxes);
        result.append(&mut self.traversal);
        result.push("}".to_string());

        result.push(String::new());
        result.join("\n")
    }

    pub fn set_note(&mut self, note: &str) -> &mut Self {
        self.note = String::from(note);

        self
    }
}

impl Node {
    pub fn to_dot(&self, index: TreeIndex) -> DotLines {
        // TODO: can this be done without introducing a blank line?
        let node_to_parent = match self.parent().0 {
            Some(parent) => format!("node_{index} -> node_{parent} [constraint=false]"),
            None => String::new(),
        };

        match self {
            Node::Internal(InternalNode { left, right, .. }) => DotLines {
                nodes: vec![format!("node_{index} [label=\"{index}\"]")],
                connections: vec![
                    format!("node_{index} -> node_{left};"),
                    format!("node_{index} -> node_{right};"),
                    node_to_parent,
                ],
                pair_boxes: vec![format!(
                    "subgraph cluster_node_{index}_children {{ style=invis; {{rank = same; node_{left}->node_{right}[style=invis]; rankdir = LR}} }}"
                )],
                note: String::new(),
                ..Default::default()
            },
            Node::Leaf(LeafNode { key, value, .. }) => DotLines {
                nodes: vec![format!(
                    "node_{index} [shape=box, label=\"{index}\\nkey: {key}\\nvalue: {value}\"];"
                )],
                connections: vec![node_to_parent],
                note: String::new(),
                ..Default::default()
            },
        }
    }
}

impl MerkleBlob {
    pub fn to_dot(&self) -> Result<DotLines, Error> {
        let mut result = DotLines::new();
        for item in LeftChildFirstIterator::new(&self.blob, None) {
            let (index, block) = item?;
            result.push(block.node.to_dot(index));
        }

        Ok(result)
    }
}

// TODO: better conditional execution than the commenting i'm doing now
#[allow(unused)]
pub fn open_dot(lines: &mut DotLines) {
    let mut url = Url::parse("http://edotor.net").unwrap();
    // https://edotor.net/?engine=dot#graph%20%7B%7D%0A -> graph {}
    url.query_pairs_mut().append_pair("engine", "dot");
    url.set_fragment(Some(
        &utf8_percent_encode(&lines.dump(), NON_ALPHANUMERIC).to_string(),
    ));
    open::that(url.as_str()).unwrap();
}
