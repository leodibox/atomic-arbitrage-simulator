use alloy::primitives::U256;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Position {
    pub pair: alloy::primitives::Address,
    pub token: alloy::primitives::Address,
    pub amount: U256,
    pub opened_block: u64,
}

pub struct Keeper {
    open: VecDeque<Position>,
    max_age_blocks: u64,
}

impl Keeper {
    pub fn new(max_age_blocks: u64) -> Self { Self { open: VecDeque::new(), max_age_blocks } }
    pub fn track(&mut self, p: Position) { self.open.push_back(p); }
    pub fn to_unwind(&mut self, current_block: u64) -> Vec<Position> {
        let mut out = Vec::new();
        while let Some(p) = self.open.front() {
            if current_block.saturating_sub(p.opened_block) >= self.max_age_blocks {
                out.push(self.open.pop_front().unwrap());
            } else { break; }
        }
        out
    }
}
