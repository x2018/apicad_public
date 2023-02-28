use llir::Module;
use std::collections::HashMap;
use std::path::PathBuf;
use structopt::StructOpt;

use analyzer::{call_graph::*, feature_extraction::*, options::*, slicer::*, symbolic_execution::*, utils::*};

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "analyzer")]
pub struct Options {
    //************************** BasicOption & GeneralOptions & IOOption **************************//
    #[structopt(index = 1, required = true, value_name = "INPUT")]
    pub input: String,

    #[structopt(index = 2, required = true, value_name = "OUTPUT")]
    pub output: String,

    #[structopt(long, takes_value = true, value_name = "SUBFOLDER")]
    pub subfolder: Option<String>,

    // Print call graph
    #[structopt(long)]
    pub print_call_graph: bool,

    #[structopt(long)]
    pub print_options: bool,

    // Serialize execution rather than parallel
    #[structopt(short = "s", long)]
    pub use_serial: bool,

    // Use batch execution. Especially useful when applying to large dataset
    #[structopt(long)]
    pub use_batch: bool,

    // The number of slices inside each batch
    #[structopt(long, takes_value = true, default_value = "50", value_name = "BATCH_SIZE")]
    pub batch_size: usize,

    // The file path for dumping the metadata of generated traces
    #[structopt(long, takes_value = true, value_name = "METADATA_FILE")]
    pub metadata_file: Option<String>,

    // The file path for dumping target-num-slices-map
    #[structopt(long, takes_value = true, value_name = "TARGET_NUM_SLICES_MAP")]
    pub target_num_slices_map_file: Option<String>,

    #[structopt(long)]
    pub no_feature: bool,

    #[structopt(long)]
    pub feature_only: bool,
    //************************** BasicOption & GeneralOptions & IOOption **************************//

    //***************************************** SliceOptions *************************************//
    #[structopt(
        short = "d",
        long,
        takes_value = true,
        default_value = "1",
        value_name = "SLICE_DEPTH"
    )]
    pub slice_depth: usize,

    // Maximum number of blocks per slice
    #[structopt(long, takes_value = true, default_value = "1000", value_name = "MAX_AVG_NUM_BLOCKS")]
    pub max_num_blocks: usize,

    // Use regex in the filters
    #[structopt(long)]
    pub use_regex_filter: bool,

    #[structopt(long, takes_value = true, value_name = "INCLUDE_TARGET")]
    pub target_inclusion_filter: Option<Vec<String>>,

    #[structopt(long, takes_value = true, value_name = "EXCLUDE_TARGET")]
    pub target_exclusion_filter: Option<Vec<String>>,
    //***************************************** SliceOptions *************************************//

    //*********************************** SymbolicExecutionOptions *******************************//
    // Maximum time per work can execute
    #[structopt(long, takes_value = true, default_value = "5", value_name = "MAX_TIMEOUT")]
    pub max_timeout: usize,

    #[structopt(long, takes_value = true, default_value = "5000", value_name = "MAX_NODE_PER_TRACE")]
    pub max_node_per_trace: usize,

    #[structopt(
        long,
        takes_value = true,
        default_value = "1000",
        value_name = "MAX_EXPLORED_TRACE_PER_SLICE"
    )]
    pub max_explored_trace_per_slice: usize,

    // The maximum number of generated trace per slice
    #[structopt(long, takes_value = true, default_value = "50", value_name = "MAX_TRACE_PER_SLICE")]
    pub max_trace_per_slice: usize,

    // Step into the calls even if the slice depth is zero
    #[structopt(long)]
    pub step_in_anytime: bool,

    // Roughly exploring without checking SAT
    #[structopt(long)]
    pub rough_mode: bool,

    // Random schedule the execution work
    #[structopt(long)]
    pub not_random_scheduling: bool,
    //*********************************** SymbolicExecutionOptions *******************************//
}

impl GeneralOptions for Options {
    fn use_serial(&self) -> bool {
        self.use_serial
    }

    fn use_batch(&self) -> bool {
        self.use_batch
    }
}

impl IOOptions for Options {
    fn input_path(&self) -> PathBuf {
        PathBuf::from(&self.input)
    }

    fn output_path(&self) -> PathBuf {
        PathBuf::from(&self.output)
    }

    fn basename_of_bc_file(&self) -> Option<&str> {
        match &self.subfolder {
            Some(subfolder) => Some(&subfolder),
            None => None,
        }
    }
}

impl Options {
    fn metadata_file_path(&self) -> Option<PathBuf> {
        if let Some(filename) = &self.metadata_file {
            Some(self.output_path().join(filename))
        } else {
            None
        }
    }

    fn target_num_slices_map_path(&self) -> Option<PathBuf> {
        if let Some(filename) = &self.target_num_slices_map_file {
            Some(self.output_path().join(filename))
        } else {
            None
        }
    }

    fn num_slices(&self, target: &str) -> usize {
        match std::fs::read_dir(self.slice_target_dir(target)) {
            Ok(dirs) => dirs.count(),
            _ => 0,
        }
    }
}

impl SlicerOptions for Options {
    fn slice_depth(&self) -> usize {
        self.slice_depth as usize
    }

    fn target_inclusion_filter(&self) -> &Option<Vec<String>> {
        &self.target_inclusion_filter
    }

    fn target_exclusion_filter(&self) -> &Option<Vec<String>> {
        &self.target_exclusion_filter
    }

    fn use_regex_filter(&self) -> bool {
        self.use_regex_filter
    }

    fn max_num_blocks(&self) -> usize {
        self.max_num_blocks
    }
}

impl SymbolicExecutionOptions for Options {
    fn slice_depth(&self) -> usize {
        self.slice_depth
    }

    fn max_timeout(&self) -> usize {
        self.max_timeout
    }

    fn max_node_per_trace(&self) -> usize {
        self.max_node_per_trace
    }

    fn max_explored_trace_per_slice(&self) -> usize {
        self.max_explored_trace_per_slice
    }

    fn max_trace_per_slice(&self) -> usize {
        self.max_trace_per_slice
    }

    fn step_in_anytime(&self) -> bool {
        self.step_in_anytime
    }

    fn is_rough(&self) -> bool {
        self.rough_mode
    }

    fn not_random_scheduling(&self) -> bool {
        self.not_random_scheduling
    }
}

impl FeatureExtractorOptions for Options {}

fn main() -> Result<(), String> {
    let options = Options::from_args();
    if options.print_options {
        println!("{:?}", options);
    }

    // Load a logging context
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
    if options.print_call_graph {
        call_graph.print();
    }

    // Finding call edges
    logging_ctx.log_finding_call_edges()?;
    let target_edges_map = TargetEdgesMap::from_call_graph(&call_graph, &options)?;

    // Check if we need to do the symbolic execution
    let occurrences = if !options.feature_only {
        // Generate slices from the edges
        logging_ctx.log_generated_call_edges(target_edges_map.num_elements())?;
        let target_slices_map = TargetSlicesMap::from_target_edges_map(&target_edges_map, &call_graph, &options);
        let occurrences = keyed_num_elements(&target_slices_map, &llmod);
        // Dump slices to file
        logging_ctx.log_generated_slices(target_slices_map.num_elements())?;
        target_slices_map.dump(&options);

        // Divide target slices into batches
        let num_batches = 1 + target_slices_map.num_elements() / options.batch_size;
        let batchmap = target_slices_map.batches(options.use_batch, options.batch_size);
        logging_ctx.log_dividing_batches(options.use_batch, num_batches)?;
        let mut global_metadata = MetaData::new();
        for (i, target_slices_map) in batchmap {
            logging_ctx.log_executing_batch(
                i,
                num_batches,
                options.use_batch,
                target_slices_map.len(),
                target_slices_map.num_elements(),
            )?;
            // Symbolic execution
            let sym_exec_ctx = SymbolicExecutionContext::new(&options);
            let metadata = sym_exec_ctx.execute_target_slices_map(target_slices_map);
            global_metadata = global_metadata.combine(metadata.clone());
            logging_ctx.log_finished_execution_batch(i, options.use_batch, metadata)?;
        }
        logging_ctx.log_finished_execution(options.use_batch, &global_metadata)?;
        // Save metadata to a temporary file
        if let Some(filename) = options.metadata_file_path() {
            global_metadata.dump(filename)?;
        }

        occurrences
    } else {
        // If not, we directly load slices information from file
        load_target_num_slices_map(target_edges_map, &llmod, &options)
    };

    // Dump the occurrences(format: {"func_name": (has_return_type, slices_num), ...}) to file
    if let Some(filename) = options.target_num_slices_map_path() {
        logging_ctx.log_dump_target_num_slices_map(&filename)?;
        occurrences.dump(filename)?;
    }

    if !options.no_feature {
        // Directly extract features
        logging_ctx.log_extracting_features()?;
        let feature_extract_ctx = FeatureExtractionContext::new(&llmod, occurrences, &options)?;
        feature_extract_ctx.extract_features(&mut logging_ctx);
        logging_ctx.log_finished_extracting_features()?;
    }

    Ok(())
}

// (func_name, (has_return_type, slices_num))
fn keyed_num_elements(target_slices_map: &TargetSlicesMap, module: &Module) -> HashMap<String, (bool, usize)> {
    let func_types = module.function_types();
    let mut result = HashMap::new();
    for (target, value) in target_slices_map {
        result.insert(
            target.clone(),
            (func_types[&target.clone()].has_return_type(), value.len()),
        );
    }
    result
}

// (func_name, (has_return_type, slices_num))
fn load_target_num_slices_map(
    target_edges_map: TargetEdgesMap,
    module: &Module,
    options: &Options,
) -> HashMap<String, (bool, usize)> {
    let func_types = module.function_types();
    target_edges_map
        .into_iter()
        .map(|(target, _)| {
            let num_slices = options.num_slices(&target);
            (target.clone(), (func_types[&target].has_return_type(), num_slices))
        })
        .collect()
}
