use llir::values::*;
use std::time::SystemTime;

use super::block_tracer::*;
use super::constraints::*;
use super::memory::*;
use super::trace::*;
use crate::semantics::rced::*;
use crate::slicer::*;

#[derive(Clone, Debug)]
pub enum FinishState {
    ProperlyReturned,
    BranchExplored,
    ExceedingMaxTraceLength,
    Unreachable,
    Timeout,
}

#[derive(Clone, Debug)]
pub struct State<'ctx> {
    pub stack: Stack<'ctx>,
    pub memory: Memory,
    pub block_trace_iter: BlockTraceIterator<'ctx>,
    pub visited_branch: VisitedBranch<'ctx>,
    pub trace: Trace<'ctx>,
    pub target_node: Option<usize>,
    pub prev_block: Option<Block<'ctx>>,
    pub finish_state: FinishState,
    pub constraints: Constraints,
    pub start_time: SystemTime,

    // Identifiers
    alloca_id: usize,
    pub symbol_id: usize,

    // The current loop depth
    pub loop_depth: usize,
    // Whether it is not on the main block path,
    // i.e., whether it is in relevant method.
    pub in_relevant_method: bool,
}

impl<'ctx> State<'ctx> {
    pub fn from_block_trace(
        slice: &Slice<'ctx>,
        block_trace: BlockTrace<'ctx>,
        max_traces_num: usize,
        not_random: bool,
    ) -> Self {
        Self {
            stack: vec![StackFrame::entry(slice.entry)],
            memory: Memory::new(),
            block_trace_iter: BlockTraceIterator::from_block_trace(
                block_trace, max_traces_num, not_random
            ),
            visited_branch: VisitedBranch::new(),
            trace: Vec::new(),
            target_node: None,
            prev_block: None,
            finish_state: FinishState::ProperlyReturned,
            constraints: Vec::new(),
            start_time: SystemTime::now(),
            alloca_id: 0,
            symbol_id: 0,
            loop_depth: 0,
            in_relevant_method: false,
        }
    }

    pub fn new_alloca_id(&mut self) -> usize {
        let result = self.alloca_id;
        self.alloca_id += 1;
        result
    }

    pub fn new_symbol_id(&mut self) -> usize {
        let result = self.symbol_id;
        self.symbol_id += 1;
        result
    }

    pub fn add_constraint(&mut self, cond: Comparison, branch: bool) {
        self.constraints.push(Constraint { cond, branch });
    }

    pub fn has_timeouted(&mut self, max_time: usize) -> bool {
        match self.start_time.elapsed() {
            Ok(elapsed) => {
                if elapsed.as_secs() as usize >= max_time {
                    return true;
                }
            }
            Err(e) => {
                // an error occurred!
                println!("Time Error: {:?}", e);
            }
        }
        false
    }
}
