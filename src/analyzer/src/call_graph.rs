use llir::{values::*, *};
use petgraph::graph::{DiGraph, EdgeIndex, Graph, NodeIndex};
use petgraph::*;
use std::collections::HashMap;

pub trait FunctionUtil<'ctx> {
    fn simp_name(&self) -> String;
}

impl<'ctx> FunctionUtil<'ctx> for Function<'ctx> {
    fn simp_name(&self) -> String {
        let name = self.name();
        match name.find('.') {
            Some(i) => {
                if &name[..i] == "llvm" {
                    match name.chars().skip(i + 2).position(|c| c == '.') {
                        Some(j) => name[i + 1..i + 2 + j].to_string(),
                        None => name[i + 1..].to_string(),
                    }
                } else {
                    name[..i].to_string()
                }
            }
            None => name,
        }
    }
}

pub struct CallEdge<'ctx> {
    pub caller: Function<'ctx>,
    pub callee: Function<'ctx>,
    pub instr: CallInstruction<'ctx>,
}

// Output formatter of call graph edge
impl<'ctx> std::fmt::Display for CallEdge<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{} -> {}: {}",
            self.caller.simp_name(),
            self.callee.simp_name(),
            self.instr.debug_loc_string(),
        ))
    }
}

fn skip_intrinsics_funcs(func_name: String) -> bool {
    let is_llvm_intrinsics = func_name.contains("llvm.") || func_name.contains("__sanitizer");
    // if func_name.contains("llvm.") {
    // 	println!("remove llvm function: {}, {}", func_name, is_llvm_intrinsics);
    // }
    !is_llvm_intrinsics
}

// CallGraph is defined by function vertices + instruction edges connecting caller & callee
pub type CallGraphRaw<'ctx> = DiGraph<Function<'ctx>, CallInstruction<'ctx>>;

pub trait CallGraphTrait<'ctx> {
    type Edge;

    fn call_edge(&self, edge_id: EdgeIndex) -> Option<CallEdge>;

    fn print(&self);
}

impl<'ctx> CallGraphTrait<'ctx> for CallGraphRaw<'ctx> {
    type Edge = EdgeIndex;

    fn call_edge(&self, edge_id: EdgeIndex) -> Option<CallEdge> {
        self.edge_endpoints(edge_id).map(|(caller_id, callee_id)| {
            let instr = self[edge_id];
            let caller = self[caller_id];
            let callee = self[callee_id];
            CallEdge { caller, callee, instr }
        })
    }

    fn print(&self) {
        for edge_id in self.edge_indices() {
            match self.call_edge(edge_id) {
                Some(ce) => println!("{}", ce.to_string()),
                None => {}
            }
        }
        // For single function
        for node_id in self.node_indices() {
            if self.edges_directed(node_id, Direction::Incoming).count() == 0
                && self.edges_directed(node_id, Direction::Outgoing).count() == 0
            {
                println!("{}", self[node_id].simp_name());
            }
        }
    }
}

pub type FunctionIdMap<'ctx> = HashMap<Function<'ctx>, NodeIndex>;

#[derive(Debug, Clone)]
pub struct GraphPath<N, E>
where
    N: Clone,
    E: Clone,
{
    pub begin: N,
    pub succ: Vec<(E, N)>,
}

impl<N, E> GraphPath<N, E>
where
    N: Clone,
    E: Clone,
{
    // Get the last element in the path.
    pub fn last(&self) -> &N {
        if self.succ.is_empty() {
            &self.begin
        } else {
            &self.succ[self.succ.len() - 1].1
        }
    }

    pub fn visited(&self, n: N) -> bool
    where
        N: Eq,
    {
        if self.begin == n {
            true
        } else {
            self.succ.iter().find(|(_, other_n)| other_n.clone() == n).is_some()
        }
    }

    // Push an element into the back of the path.
    pub fn push(&mut self, e: E, n: N) {
        self.succ.push((e, n));
    }

    // The length of the path. e.g. If a path contains 5 nodes, then the length is 4.
    pub fn len(&self) -> usize {
        self.succ.len()
    }
}

pub type IndexedGraphPath = GraphPath<NodeIndex, EdgeIndex>;

impl IndexedGraphPath {
    pub fn into_elements<N, E>(&self, graph: &Graph<N, E>) -> GraphPath<N, E>
    where
        N: Clone,
        E: Clone,
    {
        GraphPath {
            begin: graph[self.begin].clone(),
            succ: self
                .succ
                .iter()
                .map(|(e, n)| (graph[*e].clone(), graph[*n].clone()))
                .collect(),
        }
    }
}

pub type CallGraphPath<'ctx> = GraphPath<Function<'ctx>, CallInstruction<'ctx>>;

pub struct CallGraph<'ctx> {
    pub graph: CallGraphRaw<'ctx>,
    pub function_id_map: FunctionIdMap<'ctx>,
}

impl<'ctx> CallGraph<'ctx> {
    pub fn from_module(module: &Module<'ctx>) -> Self {
        let mut value_id_map: HashMap<Function<'ctx>, NodeIndex> = HashMap::new();

        // Generate Call Graph by iterating through all blocks & instructions for each function
        let mut cg = Graph::new();
        for caller in module.iter_functions() {
            // Remove llvm functions
            if !skip_intrinsics_funcs(caller.name()) {
                continue;
            }
            let caller_id = value_id_map
                .entry(caller)
                .or_insert_with(|| cg.add_node(caller))
                .clone();
            for b in caller.iter_blocks() {
                for i in b.iter_instructions() {
                    match i {
                        Instruction::Call(call_instr) => {
                            if !call_instr.is_intrinsic_call() {
                                match call_instr.callee_function() {
                                    Some(callee) => {
                                        let callee_id = value_id_map
                                            .entry(callee)
                                            .or_insert_with(|| cg.add_node(callee))
                                            .clone();
                                        cg.add_edge(caller_id, callee_id, call_instr);
                                    }
                                    None => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Return the call graph
        Self {
            graph: cg,
            function_id_map: value_id_map,
        }
    }

    pub fn print(&self) {
        self.graph.print()
    }
}
