use petgraph::*;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use structopt::StructOpt;

use analyzer::{call_graph::*, options::*, utils::*};
use llir::{types::*, values::*};

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "occurrence", about = "get functions occur in a *.bc file")]
pub struct Options {
    #[structopt(index = 1, required = true, value_name = "INPUT", about = "input file")]
    pub input: String,

    #[structopt(index = 2, required = true, value_name = "OUTPUT", about = "output file")]
    pub output: String,
}

impl IOOptions for Options {
    fn input_path(&self) -> PathBuf {
        PathBuf::from(&self.input)
    }

    fn output_path(&self) -> PathBuf {
        PathBuf::from(&self.output)
    }

    fn basename_of_bc_file(&self) -> Option<&str> {
        None
    }
}

impl Options {
    pub fn input_bc_name(&self) -> String {
        format!("{}", self.input_path().file_name().unwrap().to_str().unwrap())
    }

    fn occurrence_path(&self) -> PathBuf {
        self.output_path().join("occurrences")
    }

    fn occurrence_file_path(&self) -> PathBuf {
        let name = format!("{}", self.input_bc_name());
        self.occurrence_path().join(format!("{}.json", name))
    }
}

fn ty_str<'ctx>(t: &Type<'ctx>) -> &'static str {
    match t {
        Type::Array(_) => "[]",
        Type::Float(_) => "float",
        Type::Int(_) => "int",
        Type::Pointer(_) => "*",
        Type::Struct(_) => "{}",
        Type::Vector(_) => "()",
        Type::Void(_) => "void",
        _ => "",
    }
}

fn signature<'ctx>(f: &Function<'ctx>) -> String {
    format!(
        "{} {}({})",
        ty_str(&f.get_function_type().return_type()),
        f.simp_name(),
        f.get_function_type()
            .argument_types()
            .iter()
            .map(|t| ty_str(t).to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn main() -> Result<(), String> {
    let options = Options::from_args();
    let mut logging_ctx = LoggingContext::new(&options)?;

    // Load the byte code module and generate analyzer context
    logging_ctx.log_loading_bc()?;
    let llctx = llir::Context::create();
    let llmod = llctx
        .load_module(&options.input_path())
        .map_err(|err| err.to_string())?;

    // Generate call graph
    logging_ctx.log_generating_call_graph()?;
    let call_graph = CallGraph::from_module(&llmod);

    // Generate occurrence map
    logging_ctx.log_generating_occurrence_map()?;
    let mut map = HashMap::new();
    for node_id in call_graph.graph.node_indices() {
        let func = call_graph.graph[node_id];
        let num_call_sites = call_graph.graph.edges_directed(node_id, Direction::Incoming).count();
        *map.entry(func).or_insert(0) += num_call_sites;
    }

    // Transform occurrence map into json
    logging_ctx.log_transforming_occurrence_map2json()?;
    std::fs::create_dir_all(options.occurrence_path()).expect("Cannot create occurrence directory");
    let json_map: serde_json::Map<_, _> = map
        .into_iter()
        .map(|(func, num_call_sites)| (signature(&func), json!(num_call_sites)))
        .collect();
    let json_obj = serde_json::Value::Object(json_map);
    dump_json(&json_obj, options.occurrence_file_path())
}
