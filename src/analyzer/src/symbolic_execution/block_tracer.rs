use llir::values::*;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::collections::HashMap;
use std::time::SystemTime;

use crate::call_graph::*;
use crate::slicer::*;
use crate::utils;

/// The block trace inside a function.
///
/// Fields:
/// - function: The function that contains all the blocks
/// - block_traces: The list of traces that can go from starting block to the
///   block that contains the target call
/// - call: The final Call Instruction that leads us to the next function
///   or the target function call
#[derive(Debug)]
pub struct CompositeFunctionBlockTraces<'ctx> {
    function: Function<'ctx>,
    block_traces: Vec<Vec<Block<'ctx>>>,
    call_instr: CallInstruction<'ctx>,
}

/// A block trace is a list of FunctionBlockTrace
pub type CompositeBlockTrace<'ctx> = Vec<CompositeFunctionBlockTraces<'ctx>>;

/// One block trace inside a function leading to the call instruction
#[derive(Debug, Clone)]
pub struct FunctionBlockTrace<'ctx> {
    pub function: Function<'ctx>,
    pub block_trace: Vec<Block<'ctx>>,
    pub call_instr: CallInstruction<'ctx>,
}

/// Block trace is an array of function block trace
pub type BlockTrace<'ctx> = Vec<FunctionBlockTrace<'ctx>>;

pub trait GenerateBlockTraceTrait<'ctx> {
    fn block_traces(&self) -> Vec<BlockTrace<'ctx>>;
}

impl<'ctx> GenerateBlockTraceTrait<'ctx> for CompositeBlockTrace<'ctx> {
    fn block_traces(&self) -> Vec<BlockTrace<'ctx>> {
        if self.len() == 0 {
            vec![]
        } else {
            let func_num_block_traces: Vec<usize> = self
                .iter()
                .map(|func_blk_trace| func_blk_trace.block_traces.len())
                .collect();
            let num_block_traces = func_num_block_traces.iter().product();
            let mut block_traces = Vec::with_capacity(num_block_traces);
            let cartesian = utils::cartesian(&func_num_block_traces);
            for indices in cartesian {
                let block_trace = indices
                    .iter()
                    .enumerate()
                    .filter_map(|(i, j)| {
                        if i < self.len() && *j < self[i].block_traces.len() {
                            Some(FunctionBlockTrace {
                                function: self[i].function,
                                block_trace: self[i].block_traces[*j].clone(),
                                call_instr: self[i].call_instr,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();
                block_traces.push(block_trace);
            }
            block_traces
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlockTraceIterator<'ctx> {
    pub block_trace: BlockTrace<'ctx>,
    pub function_id: usize,
    pub block_id: usize,
    pub max_traces_num: usize,
    pub rng: StdRng,
    pub not_random: bool,
}

impl<'ctx> BlockTraceIterator<'ctx> {
    pub fn from_block_trace(block_trace: BlockTrace<'ctx>, max_traces_num: usize, not_random: bool) -> Self {
        let rng = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => StdRng::seed_from_u64(n.as_secs()),
            Err(_) => StdRng::seed_from_u64(996996),
        };
        Self {
            block_trace,
            function_id: 0,
            block_id: 0,
            max_traces_num: max_traces_num,
            rng: rng,
            not_random: not_random,
        }
    }

    pub fn visit_call(&mut self, instr: CallInstruction<'ctx>) -> bool {
        if self.function_id < self.block_trace.len() {
            if self.block_trace[self.function_id].call_instr == instr {
                self.function_id += 1;
                self.block_id = 0;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn visit_block(&mut self, prev_block: Block<'ctx>, next_block: Block<'ctx>, visit: bool) -> bool {
        if self.function_id < self.block_trace.len() {
            let block_trace = &self.block_trace[self.function_id].block_trace;
            if self.block_id < block_trace.len() - 1 && block_trace.len() != 0 {
                if block_trace[self.block_id] == prev_block && block_trace[self.block_id + 1] == next_block {
                    if visit {
                        self.block_id += 1;
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn get_junc_blk(&mut self, start_blk: Block<'ctx>) -> (Block<'ctx>, Vec<Block<'ctx>>) {
        let mut added_block: HashMap<Block, usize> = HashMap::new();
        added_block.insert(start_blk, 0);
        let ori_blk_traces = &self.block_trace[self.function_id].block_trace[self.block_id + 2..];
        let mut fringe = Vec::new();

        fringe.push((start_blk, vec![]));
        while !fringe.is_empty() {
            if !self.not_random {
                let idx = self.rng.gen_range(0, fringe.len());
                let last_idx = fringe.len() - 1;
                fringe.swap(idx, last_idx);
            }
            let (curr_blk, blk_nodes) = fringe.pop().unwrap();
            for blk in curr_blk.destination_blocks() {
                added_block.entry(blk).or_insert(0);
                if let Some(value) = added_block.get_mut(&blk) {
                    if *value < self.max_traces_num || blk.is_loop_entry_block() {
                        *value += 1;
                        if ori_blk_traces.contains(&blk) {
                            return (blk, blk_nodes);
                        }
                        let mut new_blk_nodes = blk_nodes.clone();
                        new_blk_nodes.push(blk);
                        fringe.insert(0, (blk, new_blk_nodes));
                    }
                }
            }
        }

        (start_blk, vec![])
    }

    /* Correct the pre-collected blk_path if there is an obviously infeasible path */
    pub fn correct_blk_paths(&mut self, new_block: Block<'ctx>) -> bool {
        if self.function_id > self.block_trace.len() {
            return false;
        } else if self.block_id >= self.block_trace[self.function_id].block_trace.len() - 2 {
            return false;
        }

        let (junc_blk, new_nodes) = self.get_junc_blk(new_block);
        if junc_blk == new_block {
            return false;
        }
        let mut index = self.block_id + 1;
        self.block_trace[self.function_id].block_trace[index] = new_block;

        // Remove original intermediate nodes
        while index < self.block_trace[self.function_id].block_trace.len() - 2 {
            if self.block_trace[self.function_id].block_trace[index + 1] != junc_blk {
                self.block_trace[self.function_id].block_trace.remove(index + 1);
            } else {
                break;
            }
        }

        // Add new intermediate nodes
        for node in new_nodes {
            index += 1;
            self.block_trace[self.function_id].block_trace.insert(index, node);
        }

        true
    }
}

pub struct BlockGraph<'ctx> {
    graph: DiGraph<Block<'ctx>, Instruction<'ctx>>,
    block_id_map: HashMap<Block<'ctx>, NodeIndex>,
    entry_id: NodeIndex,
    max_traces_num: usize,
    rng: StdRng,
    not_random: bool,
}

impl<'ctx> BlockGraph<'ctx> {
    pub fn reverse_search_blk_traces(&mut self, target_block: Block<'ctx>) -> Vec<Vec<Block<'ctx>>> {
        // We assume the first block of the function will be finally fetched.
        let mut blk_traces = Vec::new();
        let target_id = self.block_id_map[&target_block];
        let mut visited_block: HashMap<NodeIndex, usize> = HashMap::new();
        visited_block.insert(target_id, self.max_traces_num);

        // Generate block traces
        let mut fringe = Vec::new();
        fringe.push((target_id, vec![self.graph[target_id]]));
        while !fringe.is_empty() && blk_traces.len() < self.max_traces_num {
            if !self.not_random {
                let idx = self.rng.gen_range(0, fringe.len());
                let last_idx = fringe.len() - 1;
                fringe.swap(idx, last_idx);
            }
            let (curr_block_id, blk_trace) = fringe.pop().unwrap();
            for block_id in self.graph.neighbors_directed(curr_block_id, Direction::Incoming) {
                // We directly set every block can be accessed `max_traces_num` times.
                // i.e. every block can appear in `max_traces_num` different traces at most.
                visited_block.entry(block_id).or_insert(0);
                if let Some(value) = visited_block.get_mut(&block_id) {
                    if *value < self.max_traces_num || self.graph[block_id].is_loop_entry_block() {
                        *value += 1;
                        if !blk_trace.contains(&self.graph[block_id]) || self.graph[block_id].is_loop_entry_block() {
                            let mut new_blk_trace = blk_trace.clone();
                            new_blk_trace.insert(0, self.graph[block_id]);
                            if block_id == self.entry_id && !blk_traces.contains(&new_blk_trace) {
                                blk_traces.push(new_blk_trace);
                                continue;
                            }
                            fringe.push((block_id, new_blk_trace));
                        }
                    }
                }
            }
        }
        blk_traces
    }
}

pub trait FunctionBlockGraphTrait<'ctx> {
    fn block_graph(&self, entry: Block<'ctx>, max_traces_num: usize, not_random: bool) -> BlockGraph<'ctx>;

    fn block_traces_to_instr(
        &self,
        instr: Instruction<'ctx>,
        max_traces_num: usize,
        not_random: bool,
    ) -> Vec<Vec<Block<'ctx>>>;
}

impl<'ctx> FunctionBlockGraphTrait<'ctx> for Function<'ctx> {
    fn block_graph(&self, entry: Block<'ctx>, max_traces_num: usize, not_random: bool) -> BlockGraph<'ctx> {
        let mut block_id_map = HashMap::new();
        let mut graph = DiGraph::new();
        for block in self.iter_blocks() {
            let block_id = block_id_map
                .entry(block)
                .or_insert_with(|| graph.add_node(block))
                .clone();
            let terminator = block.last_instruction().unwrap();
            let next_blocks = block.destination_blocks();
            for next_block in next_blocks {
                let next_block_id = block_id_map
                    .entry(next_block)
                    .or_insert_with(|| graph.add_node(next_block))
                    .clone();
                graph.add_edge(block_id, next_block_id, terminator);
            }
        }
        let entry_id = block_id_map[&entry];
        let rng = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => StdRng::seed_from_u64(n.as_secs()),
            Err(_) => StdRng::seed_from_u64(996996),
        };
        BlockGraph {
            graph,
            block_id_map,
            entry_id,
            max_traces_num,
            rng,
            not_random,
        }
    }

    fn block_traces_to_instr(
        &self,
        instr: Instruction<'ctx>,
        max_traces_num: usize,
        not_random: bool,
    ) -> Vec<Vec<Block<'ctx>>> {
        match self.first_block() {
            None => vec![vec![]],
            _ => {
                let entry_block = self.first_block().unwrap();
                if entry_block == instr.parent_block() {
                    vec![vec![entry_block]]
                } else {
                    let mut block_graph = self.block_graph(entry_block, max_traces_num, not_random);
                    if block_graph.block_id_map.contains_key(&instr.parent_block()) {
                        block_graph.reverse_search_blk_traces(instr.parent_block())
                    } else {
                        vec![vec![]]
                    }
                }
            }
        }
    }
}

pub trait BlockTracesFromCallGraphPath<'ctx> {
    fn block_traces(&self, max_traces_num: usize, not_random: bool) -> Vec<BlockTrace<'ctx>>;
}

impl<'ctx> BlockTracesFromCallGraphPath<'ctx> for CallGraphPath<'ctx> {
    fn block_traces(&self, max_traces_num: usize, not_random: bool) -> Vec<BlockTrace<'ctx>> {
        let mut curr_func = self.begin;
        let mut comp_trace = vec![];
        for (call_instr, next_func) in &self.succ {
            // Target-oriented block traces reverse!
            let block_traces = curr_func.block_traces_to_instr(call_instr.as_instruction(), max_traces_num, not_random);
            comp_trace.push(CompositeFunctionBlockTraces {
                function: curr_func,
                block_traces,
                call_instr: call_instr.clone(),
            });
            curr_func = next_func.clone();
        }
        comp_trace.block_traces()
    }
}

pub trait BlockTracesFromSlice<'ctx> {
    fn block_traces(&self, max_traces_num: usize, not_random: bool) -> Vec<BlockTrace<'ctx>>;
}

impl<'ctx> BlockTracesFromSlice<'ctx> for Slice<'ctx> {
    fn block_traces(&self, max_traces_num: usize, not_random: bool) -> Vec<BlockTrace<'ctx>> {
        let mut traces = vec![];
        traces.extend(self.call_chain.block_traces(max_traces_num * 2, not_random));
        traces
    }
}
