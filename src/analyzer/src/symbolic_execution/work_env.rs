use llir::values::*;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::SystemTime;

use crate::slicer::*;
use crate::symbolic_execution::*;

#[derive(Clone, Debug)]
pub struct Work<'ctx> {
    pub block: Block<'ctx>,
    pub state: State<'ctx>,
}

impl<'ctx> Work<'ctx> {
    pub fn entry_with_block_trace(
        slice: &Slice<'ctx>,
        block_trace: BlockTrace<'ctx>,
        max_traces_num: usize,
        not_random: bool,
    ) -> Self {
        let block = slice.entry.first_block().unwrap();
        let state = State::from_block_trace(slice, block_trace, max_traces_num, not_random);
        Self { block, state }
    }

    pub fn new(block: Block<'ctx>, state: State<'ctx>) -> Self {
        Self { block, state }
    }
}

pub struct Environment<'ctx> {
    pub slice: Slice<'ctx>,
    pub work_list: Vec<Work<'ctx>>,
    pub block_traces: Vec<Vec<Block<'ctx>>>,
    pub call_id: usize,
    pub is_rough: bool,
    pub rng: StdRng,
}

impl<'ctx> Environment<'ctx> {
    pub fn new(slice: &Slice<'ctx>, is_rough_mode: bool) -> Self {
        Self {
            slice: slice.clone(),
            work_list: vec![],
            block_traces: vec![],
            call_id: 0,
            is_rough: is_rough_mode,
            rng: match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                Ok(n) => StdRng::seed_from_u64(n.as_secs()),
                Err(_) => StdRng::seed_from_u64(996996),
            },
        }
    }

    pub fn is_rough_mode(&self) -> bool {
        self.is_rough
    }

    pub fn change_to_rough(&mut self) {
        self.is_rough = true;
    }

    pub fn has_work(&self) -> bool {
        !self.work_list.is_empty()
    }

    pub fn pop_work(&mut self, not_random: bool) -> Work<'ctx> {
        if !not_random {
            let idx = self.rng.gen_range(0, self.work_list.len());
            let last_idx = self.work_list.len() - 1;
            self.work_list.swap(idx, last_idx);
        }
        self.work_list.pop().unwrap()
    }

    pub fn add_work(&mut self, work: Work<'ctx>) -> bool {
        self.work_list.push(work);
        true
    }

    pub fn new_call_id(&mut self) -> usize {
        let result = self.call_id;
        self.call_id += 1;
        result
    }

    pub fn add_block_trace(&mut self, block_trace: Vec<Block<'ctx>>) {
        self.block_traces.push(block_trace)
    }

    pub fn has_duplicate(&self, block_trace: &Vec<Block<'ctx>>) -> bool {
        for other_block_trace in self.block_traces.iter() {
            if block_trace.equals(other_block_trace) {
                return true;
            }
        }
        false
    }
}

pub trait BlockTraceComparison {
    fn equals(&self, other: &Self) -> bool;
}

impl<'ctx> BlockTraceComparison for Vec<Block<'ctx>> {
    fn equals(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            false
        } else {
            for i in 0..self.len() {
                if self[i] != other[i] {
                    return false;
                }
            }
            true
        }
    }
}
