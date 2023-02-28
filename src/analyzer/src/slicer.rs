use llir::values::*;
use petgraph::{graph::*, visit::*, Direction};
use rayon::prelude::*;
use regex::Regex;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::call_graph::*;
use crate::options::*;
use crate::utils::*;

pub trait SlicerOptions: GeneralOptions + Send + Sync {
    fn slice_depth(&self) -> usize;

    fn target_inclusion_filter(&self) -> &Option<Vec<String>>;

    fn target_exclusion_filter(&self) -> &Option<Vec<String>>;

    fn use_regex_filter(&self) -> bool;

    fn max_num_blocks(&self) -> usize;
}

#[derive(Clone)]
pub struct Slice<'ctx> {
    pub entry: Function<'ctx>,
    pub caller: Function<'ctx>,
    pub call_chain: CallGraphPath<'ctx>,
    pub callee: Function<'ctx>,
    pub instr: CallInstruction<'ctx>,
    pub functions: HashSet<(Function<'ctx>, CallInstruction<'ctx>)>,
}

impl<'ctx> Slice<'ctx> {
    pub fn contains(&self, f: (Function<'ctx>, CallInstruction<'ctx>)) -> bool {
        self.functions.contains(&f)
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut call_chain = vec![];
        call_chain.push(self.call_chain.begin.simp_name());
        for (_, f) in &self.call_chain.succ {
            call_chain.push(f.simp_name());
        }
        json!({
            "entry": self.entry.simp_name(),
            "caller": self.caller.simp_name(),
            "callee": self.callee.simp_name(),
            "instr": self.instr.debug_loc_string(),
            "functions": self.functions.iter().map(|(f, instr)|
                                                    (f.simp_name(), instr.debug_loc_string()))
                                                .collect::<HashSet<_>>(),
            "call_chain": call_chain,
        })
    }

    pub fn target_function_name(&self) -> String {
        self.callee.simp_name()
    }

    pub fn size(&self) -> usize {
        self.functions.len()
    }
}

enum TargetFilter {
    Regex(Regex),
    Str(String),
    None(bool),
}

impl TargetFilter {
    pub fn new(filter_str: Option<String>, use_regex: bool, default: bool) -> Result<Self, String> {
        match filter_str {
            Some(s) => {
                if use_regex {
                    let regex = Regex::new(s.as_str()).map_err(|_| "Cannot parse target filter".to_string())?;
                    Ok(Self::Regex(regex))
                } else {
                    Ok(Self::Str(s.clone()))
                }
            }
            _ => Ok(Self::None(default)),
        }
    }

    pub fn matches(&self, f: &str) -> bool {
        // Omitting the number after `.`
        let f = match f.find('.') {
            Some(i) => &f[..i],
            None => f,
        };
        match self {
            Self::Regex(r) => r.is_match(f),
            Self::Str(s) => s == f,
            Self::None(d) => d.clone(),
        }
    }
}

// Map from function name to Edges (`Vec<Edge>`)
pub type TargetEdgesMap = HashMap<String, Vec<EdgeIndex>>;

pub trait TargetEdgesMapTrait: Sized {
    fn from_call_graph<'ctx>(call_graph: &CallGraph<'ctx>, options: &impl SlicerOptions) -> Result<Self, String>;
}

impl TargetEdgesMapTrait for TargetEdgesMap {
    fn from_call_graph<'ctx>(call_graph: &CallGraph<'ctx>, options: &impl SlicerOptions) -> Result<Self, String> {
        let mut inclusion_filters = vec![];
        let mut exclusion_filters = vec![];
        let target_functions = match options.target_inclusion_filter() {
            Some(value) => value.clone(),
            None => vec![],
        };
        let ignore_functions = match options.target_exclusion_filter() {
            Some(value) => value.clone(),
            None => vec![],
        };
        for target_func in target_functions {
            let inclusion_filter = TargetFilter::new(Some(target_func.to_string()), options.use_regex_filter(), true)?;
            inclusion_filters.push(inclusion_filter);
        }
        for ignore_func in ignore_functions {
            let exclusion_filter = TargetFilter::new(Some(ignore_func.to_string()), options.use_regex_filter(), false)?;
            exclusion_filters.push(exclusion_filter);
        }
        let mut target_edges_map = TargetEdgesMap::new();
        for callee_id in call_graph.graph.node_indices() {
            let func = call_graph.graph[callee_id];
            let func_name = func.simp_name();
            let mut include_from_inclusion = if inclusion_filters.len() == 0 { true } else { false };
            for inclusion_filter in &inclusion_filters {
                if inclusion_filter.matches(func_name.as_str()) {
                    include_from_inclusion = true;
                    break;
                }
            }
            let mut exclude_from_inclusion = false;
            for exclusion_filter in &exclusion_filters {
                if exclusion_filter.matches(func_name.as_str()) {
                    exclude_from_inclusion = true;
                    break;
                }
            }
            let include = if !include_from_inclusion {
                false
            } else {
                !exclude_from_inclusion
            };
            if include {
                for edge in call_graph.graph.edges_directed(callee_id, Direction::Incoming) {
                    target_edges_map
                        .entry(func_name.clone())
                        .or_insert(Vec::new())
                        .push(edge.id());
                }
            }
        }
        Ok(target_edges_map)
    }
}

// Map call edges to Slices, key = func_name, value = Vec<Slice>
pub type TargetSlicesMap<'ctx> = HashMap<String, Vec<Slice<'ctx>>>;

pub trait TargetSlicesMapTrait<'ctx>: Sized {
    fn from_target_edges_map(
        target_edges_map: &TargetEdgesMap,
        call_graph: &CallGraph<'ctx>,
        options: &impl SlicerOptions,
    ) -> Self;

    fn dump<O>(&self, options: &O)
    where
        O: SlicerOptions + IOOptions;
}

impl<'ctx> TargetSlicesMapTrait<'ctx> for TargetSlicesMap<'ctx> {
    fn from_target_edges_map(
        target_edges_map: &TargetEdgesMap,
        call_graph: &CallGraph<'ctx>,
        options: &impl SlicerOptions,
    ) -> Self {
        let mut result = HashMap::new();
        for (target, edges) in target_edges_map {
            let slices = call_graph.slices_of_call_edges(&edges[..], options);
            result.insert(target.clone(), slices);
        }
        result
    }

    fn dump<O>(&self, options: &O)
    where
        O: SlicerOptions + IOOptions,
    {
        for (target, slices) in self {
            fs::create_dir_all(options.slice_target_dir(target.as_str())).expect("Cannot create slice folder");
            slices.par_iter().enumerate().for_each(|(i, slice)| {
                let path = options.slice_target_file_path(target.as_str(), i);
                dump_json(&slice.to_json(), path).expect("Cannot dump slice json");
            });
        }
    }
}

pub type TargetNumSlicesMap = HashMap<String, (bool, usize)>;

pub trait TargetNumSlicesMapTrait {
    fn dump(&self, filename: PathBuf) -> Result<(), String>;
}

impl TargetNumSlicesMapTrait for TargetNumSlicesMap {
    fn dump(&self, filename: PathBuf) -> Result<(), String> {
        crate::utils::dump_json(
            &serde_json::Value::Object(
                self.iter()
                    .map(|(name, (has_return_type, num))| (name.clone(), json!([has_return_type, num.clone()])))
                    .collect(),
            ),
            filename,
        )
    }
}

pub trait Slicer<'ctx> {
    fn slices_of_call_edges(&self, edges: &[EdgeIndex], options: &impl SlicerOptions) -> Vec<Slice<'ctx>>;

    fn generate_slices(&self, edge_id: EdgeIndex, options: &impl SlicerOptions) -> Vec<Slice<'ctx>>;
}

impl<'ctx> Slicer<'ctx> for CallGraph<'ctx> {
    // Generate slices
    fn generate_slices(&self, edge_id: EdgeIndex, options: &impl SlicerOptions) -> Vec<Slice<'ctx>> {
        // Basic information
        let mut slice_call_chains: Vec<CallGraphPath<'ctx>> = vec![];
        let instr = self.graph[edge_id];
        let (caller_id, caller, callee) = {
            let (caller_id, callee_id) = self.graph.edge_endpoints(edge_id).unwrap();
            (caller_id, self.graph[caller_id], self.graph[callee_id])
        };

        if caller == callee || instr.debug_loc_string() == "" {
            return vec![];
        }

        // get the target function index in caller
        let mut index = self.graph.edges(caller_id).count() - 1;
        for edge in self.graph.edges(caller_id) {
            // the result is reverse with the result of itering the function...
            if edge.id() == edge_id {
                break;
            }
            index -= 1;
        }

        let mut target_is_returned = false;
        // Get directly related functions
        let related_funcs = direct_related_funcs(&self.graph[caller_id], index, &mut target_is_returned);
        // Set up the init slice depth
        let mut init_depth = options.slice_depth();
        if is_wrapper_function(&self.graph[caller_id]) // || target_is_returned
        {
            init_depth += 1;
        }

        // Generate slices
        let mut fringe = Vec::new();
        fringe.push((
            caller_id,
            init_depth,
            CallGraphPath {
                begin: caller,
                succ: vec![(instr, callee)],
            },
            vec![caller_id],
        ));
        while !fringe.is_empty() {
            let (func_id, mut depth, mut call_chain, mut callers) = fringe.pop().unwrap();
            if depth == 0 {
                slice_call_chains.push(call_chain);
            } else {
                let mut contains_parent = false;
                for incoming_edge in self.graph.edges_directed(func_id, Direction::Incoming) {
                    let new_caller_id = incoming_edge.source();
                    if !callers.contains(&new_caller_id) {
                        contains_parent = true;
                        let new_instr = self.graph[incoming_edge.id()];
                        call_chain.begin = self.graph[new_caller_id];
                        call_chain.succ.insert(0, (new_instr, self.graph[func_id]));
                        if is_wrapper_function(&self.graph[new_caller_id]) {
                            depth = depth + 1;
                        }
                        callers.push(new_caller_id);
                        fringe.push((new_caller_id, depth - 1, call_chain.clone(), callers.clone()));
                    }
                }
                // If the node is one head node of call graph
                if !contains_parent {
                    slice_call_chains.push(call_chain);
                }
            }
        }

        // Return slices
        let mut slices = Vec::new();
        for call_chain in slice_call_chains {
            let entry = call_chain.begin;
            let functions = related_funcs.iter().map(|func| *func).collect();
            let slice = Slice {
                caller,
                call_chain,
                callee,
                instr,
                entry,
                functions,
            };
            slices.push(slice)
        }
        slices
    }

    fn slices_of_call_edges(&self, edges: &[EdgeIndex], options: &impl SlicerOptions) -> Vec<Slice<'ctx>> {
        let f = |edge_id: &EdgeIndex| -> Vec<Slice<'ctx>> { self.generate_slices(edge_id.clone(), options) };
        if options.use_serial() {
            edges.iter().map(f).flatten().collect()
        } else {
            edges.par_iter().map(f).flatten().collect()
        }
    }
}

//* Begin: find related functions in caller *//
fn get_args<'ctx>(call_instr: &CallInstruction<'ctx>) -> HashSet<Operand<'ctx>> {
    let mut oprands = HashSet::new();
    for arg in call_instr.arguments() {
        match arg {
            Operand::Constant(_) => {}
            _ => {
                oprands.insert(arg);
            }
        }
    }
    oprands
}

fn direct_related_funcs<'ctx>(
    caller: &Function<'ctx>,
    index: usize,
    target_is_returned: &mut bool,
) -> HashSet<(Function<'ctx>, CallInstruction<'ctx>)> {
    let mut related_funcs: HashSet<(Function<'ctx>, CallInstruction<'ctx>)> = HashSet::new();
    let mut var_map: HashMap<Operand<'ctx>, HashSet<Operand<'ctx>>> = HashMap::new(); // {Loc: [potential_values...]}
    let mut functions = vec![]; // each call in the caller: [(function, callinstr), ...]
    let mut func_args = vec![]; // arguments of each call in the caller
    let mut func_ret = vec![]; // return of each call in the caller
    let mut caller_ret = HashSet::new(); // return of the caller

    // flow-insensitve: record all possible propagation locations of the variables linearly
    for b in caller.iter_blocks() {
        for instr in b.iter_instructions() {
            match instr {
                Instruction::Call(call_instr) => {
                    if !call_instr.is_intrinsic_call() {
                        match call_instr.callee_function() {
                            Some(callee) => {
                                functions.push((callee, call_instr));
                                func_args.push(get_args(&call_instr));
                                let mut ret = HashSet::new();
                                ret.insert(Operand::Instruction(instr));
                                func_ret.push(ret);
                            }
                            None => {}
                        }
                    }
                }
                // value = *loc;
                Instruction::Load(cur_instr) => {
                    for (_, val) in var_map.iter_mut() {
                        if val.contains(&cur_instr.location()) {
                            val.insert(Operand::Instruction(instr));
                        }
                    }
                }
                // value = *(loc+index);
                Instruction::GetElementPtr(cur_instr) => {
                    for (_, val) in var_map.iter_mut() {
                        if val.contains(&cur_instr.location()) {
                            val.insert(Operand::Instruction(instr));
                        }
                    }
                }
                // value = (..)op;
                Instruction::Unary(unary_instr) => {
                    for (_, val) in var_map.iter_mut() {
                        if val.contains(&unary_instr.op0()) {
                            val.insert(Operand::Instruction(instr));
                        }
                    }
                }
                // *loc = value;
                Instruction::Store(store_instr) => {
                    for (_, val) in var_map.iter_mut() {
                        // directly store to the location
                        if val.contains(&store_instr.location()) {
                            val.insert(store_instr.value());
                            // extract the internal value of unary_instr
                            match store_instr.value() {
                                Operand::Instruction(tmp_instr) => match tmp_instr {
                                    Instruction::Unary(unary_instr) => {
                                        val.insert(unary_instr.op0());
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
                        // the value is from another location
                        } else if val.contains(&store_instr.value()) {
                            val.insert(store_instr.location());
                        }
                    }
                    for ret in func_ret.iter_mut() {
                        if ret.contains(&store_instr.value()) {
                            ret.insert(store_instr.location());
                        }
                    }
                }
                // The IR is SSA form?
                Instruction::Phi(phi_instr) => {
                    if phi_instr.num_incomings() > 0 {
                        for (_, val) in var_map.iter_mut() {
                            for phiincoming in phi_instr.incomings() {
                                if val.contains(&phiincoming.value) {
                                    val.insert(Operand::Instruction(instr));
                                }
                            }
                        }
                    }
                }
                Instruction::Alloca(_) => {
                    let mut new_alloca = HashSet::new();
                    new_alloca.insert(Operand::Instruction(instr));
                    var_map.insert(Operand::Instruction(instr), new_alloca);
                }
                Instruction::Return(ret_instr) => {
                    if ret_instr.has_op() {
                        match ret_instr.op().unwrap() {
                            Operand::Constant(_) => {}
                            _ => {
                                caller_ret.insert(ret_instr.op().unwrap());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // update the potential locations
    let mut i;
    for (key, val) in var_map.iter() {
        i = 0;
        while i < functions.len() {
            let args = &mut func_args[i];
            let ret = &mut func_ret[i];
            if !val.is_disjoint(&args) {
                args.insert(*key);
            }
            if !val.is_disjoint(&ret) {
                ret.insert(*key);
            }
            i += 1;
        }
        if !val.is_disjoint(&caller_ret) {
            caller_ret.insert(*key);
        }
    }

    // determine whether the target is returned by the caller
    *target_is_returned = !caller_ret.is_disjoint(&func_ret[index]);

    // determine which are related functions
    i = 0;
    while i < functions.len() {
        if i != index {
            if !func_args[index].is_disjoint(&func_args[i]) {
                related_funcs.insert(functions[i]);
            }
            if i < index && !func_ret[i].is_disjoint(&func_args[index]) {
                related_funcs.insert(functions[i]);
            } else if !func_args[i].is_disjoint(&func_ret[index]) {
                related_funcs.insert(functions[i]);
            }
        }
        i += 1;
    }

    related_funcs
}
//* End: find related functions in caller *//

fn is_wrapper_function<'ctx>(f: &Function<'ctx>) -> bool {
    let mut blocks_num = 0;
    let mut result = false;
    for blk in f.iter_blocks() {
        if blocks_num > 1 {
            return false;
        }
        for i in blk.iter_instructions() {
            match i {
                Instruction::Return(ret_instr) => {
                    if ret_instr.has_op() {
                        match ret_instr.op().unwrap() {
                            Operand::Instruction(instr) => match instr {
                                Instruction::Call(_) => {
                                    result = true;
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    } else {
                        result = true;
                    }
                }
                Instruction::Call(call_instr) => {
                    if call_instr.is_intrinsic_call() {
                        continue;
                    }
                    if call_instr.callee_function() == None || f.name() == call_instr.callee_function().unwrap().name()
                    {
                        return false;
                    }
                    if f.num_arguments() > call_instr.num_arguments() {
                        return false;
                    }
                    let callee: Function<'ctx> = call_instr.callee_function().unwrap();
                    let func_type_of_caller = f.get_function_type();
                    let func_type_of_callee = callee.get_function_type();
                    if func_type_of_caller.return_type() != func_type_of_callee.return_type() {
                        return false;
                    }
                    let args_type_of_callee = func_type_of_callee.argument_types();
                    for arg_type in func_type_of_caller.argument_types() {
                        if !args_type_of_callee.contains(&arg_type) {
                            return false;
                        }
                    }
                }
                _ => {}
            }
        }
        blocks_num += 1;
    }
    result
}
