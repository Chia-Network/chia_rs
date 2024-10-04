// TODO: this should probably be test code?
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
