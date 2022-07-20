use std::collections::HashMap;

pub struct Module {
    pub out_devices: usize,
    pub events: Vec<HashMap<String, Block>>,
}

pub struct Block(pub Vec<Instruction>);

/// Stack operations are defined to use the top-most value on the stack as source or primary, and the below value as destination or secondary
pub enum Instruction {}
