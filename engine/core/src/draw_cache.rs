//! Bounded command replay, invalidated at tree mutation boundaries.
use crate::{spec, tree::Tree};
use alloc::vec::Vec;

const MAX_NODES: usize = 512;
const MAX_WORDS: usize = 256;
pub(crate) type Context = [u32; 15];

#[derive(Default)]
pub(crate) struct Commands {
    context: Context,
    version: u64,
    pub words: Vec<u32>,
    pub textures: Vec<i32>,
}
#[derive(Default)]
struct State {
    blocked: bool,
    commands: Commands,
}
pub(crate) struct DrawCache {
    pub enabled: bool,
    pub external_state: bool,
    active: bool,
    structure_revision: u64,
    nodes: Vec<State>,
    hits: core::cell::Cell<u32>,
}
impl DrawCache {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            external_state: false,
            active: false,
            structure_revision: 0,
            nodes: Vec::new(),
            hits: core::cell::Cell::new(0),
        }
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn info(&self) -> (bool, bool, usize, u32) {
        (
            self.enabled,
            self.active,
            self.nodes
                .iter()
                .filter(|n| n.commands.version != 0)
                .count(),
            self.hits.get(),
        )
    }
    pub fn invalidate(&mut self) {
        for state in &mut self.nodes {
            state.commands.version = 0;
        }
    }
    pub fn prepare(&mut self, tree: &Tree, allowed: bool) {
        self.active =
            self.enabled && allowed && !self.external_state && tree.slots.len() <= MAX_NODES;
        if !self.active || self.structure_revision == tree.structure_revision {
            return;
        }
        self.nodes.resize_with(tree.slots.len(), State::default);
        for state in &mut self.nodes {
            state.blocked = false;
        }
        for (slot, node) in tree.slots.iter().enumerate() {
            if !node.alive {
                self.nodes[slot] = State::default();
            } else if node.node_type > spec::NodeType::Text as u8 {
                // Image/surface content can become available without a tree mutation.
                let mut current = Some(slot as u32);
                while let Some(index) = current {
                    if self.nodes[index as usize].blocked {
                        break;
                    }
                    self.nodes[index as usize].blocked = true;
                    current = tree.resolve(tree.slots[index as usize].parent);
                }
            }
        }
        self.structure_revision = tree.structure_revision;
    }

    pub fn get(&self, slot: u32, version: u64, context: Context) -> Option<&Commands> {
        if !self.active {
            return None;
        }
        let state = &self.nodes[slot as usize];
        if !state.blocked
            && state.commands.version == version
            && version != 0
            && state.commands.context == context
        {
            self.hits.set(self.hits.get().wrapping_add(1));
            Some(&state.commands)
        } else {
            None
        }
    }
    pub fn store(&mut self, slot: u32, version: u64, context: Context, words: &[u32]) {
        if !self.active {
            return;
        }
        let state = &mut self.nodes[slot as usize];
        state.commands.version = 0;
        if state.blocked || words.len() > MAX_WORDS {
            return;
        }
        let commands = &mut state.commands;
        commands.words.clear();
        commands.words.reserve_exact(words.len());
        commands.words.extend_from_slice(words);
        commands.textures.clear();
        let mut at = 0;
        while at < words.len() {
            if matches!(words[at], spec::draw_op::TEX_QUAD | spec::draw_op::TEX_TRI) {
                let texture = words[at + 1] as i32;
                if !commands.textures.contains(&texture) {
                    commands.textures.push(texture);
                }
            }
            at += pocket_render::word_len(words, at).expect("core emits complete draw operations");
        }
        commands.context = context;
        commands.version = version;
    }
}
