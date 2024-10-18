use crate::merkle::{MerkleBlob, Node, NodeSpecific, TreeIndex};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use url::Url;

pub struct DotLines {
    pub nodes: Vec<String>,
    pub connections: Vec<String>,
    pub pair_boxes: Vec<String>,
    pub note: String,
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
            note: String::new(),
        }
    }

    pub fn push(&mut self, mut other: DotLines) {
        self.nodes.append(&mut other.nodes);
        self.connections.append(&mut other.connections);
        self.pair_boxes.append(&mut other.pair_boxes);
    }

    pub fn dump(&mut self) -> String {
        // TODO: consuming itself, secretly
        let note = &self.note;
        let mut result = vec![format!("# {note}"), String::new(), "digraph {".to_string()];
        result.append(&mut self.nodes);
        result.append(&mut self.connections);
        result.append(&mut self.pair_boxes);
        result.push("}".to_string());

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
        let node_to_parent = match self.parent {
            Some(parent) => format!("node_{index} -> node_{parent};"),
            None => String::new(),
        };

        match self.specific {
            NodeSpecific::Internal {left, right} => DotLines{
                nodes: vec![
                    format!("node_{index} [label=\"{index}\"]"),
                ],
                connections: vec![
                    format!("node_{index} -> node_{left};"),
                    format!("node_{index} -> node_{right};"),
                    node_to_parent,
                ],
                pair_boxes: vec![
                    format!("node [shape = box]; {{rank = same; node_{left}->node_{right}[style=invis]; rankdir = LR}}"),
                ],
                note: String::new(),
            },
            NodeSpecific::Leaf {key, value} => DotLines{
                nodes: vec![
                    format!("node_{index} [shape=box, label=\"{index}\\nvalue: {key}\\nvalue: {value}\"];"),
                ],
                connections: vec![node_to_parent],
                pair_boxes: vec![],
                note: String::new(),
            },
        }
    }
}

impl MerkleBlob {
    pub fn to_dot(&self) -> DotLines {
        let mut result = DotLines::new();
        for (index, block) in self {
            result.push(block.node.to_dot(index));
        }

        result
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
