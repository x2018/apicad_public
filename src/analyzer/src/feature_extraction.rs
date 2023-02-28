use indicatif::*;
use llir::{types::*, Module};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::call_graph::FunctionUtil;
use crate::feature_extractors::*;
use crate::options::*;
use crate::semantics::boxed::*;
use crate::utils::*;

#[derive(Deserialize)]
pub struct Slice {
    pub instr: String,
    pub entry: String,
    pub caller: String,
    pub callee: String,
    pub functions: Vec<(String, String)>,
}

impl Slice {}

#[derive(Deserialize)]
pub struct Instr {
    pub loc: String,
    pub sem: Semantics,
    pub res: Option<Value>,
}

#[derive(Deserialize)]
pub struct Trace {
    pub target: usize,
    pub instrs: Vec<Instr>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TraceIterDirection {
    Forward,
    Backward,
}

impl TraceIterDirection {
    pub fn is_forward(&self) -> bool {
        self == &Self::Forward
    }
}

impl Trace {
    pub fn target_result(&self) -> &Option<Value> {
        &self.target_instr().res
    }

    pub fn target_instr(&self) -> &Instr {
        &self.instrs[self.target]
    }

    pub fn target_args(&self) -> Vec<&Value> {
        self.target_instr().sem.call_args()
    }

    pub fn iter_instrs_from_target(&self, dir: TraceIterDirection) -> Vec<(usize, &Instr)> {
        self.iter_instrs_from(dir, self.target)
    }

    pub fn iter_instrs_from(&self, dir: TraceIterDirection, from: usize) -> Vec<(usize, &Instr)> {
        if dir.is_forward() {
            self.instrs.iter().enumerate().skip(from + 1).collect::<Vec<_>>()
        } else {
            self.instrs.iter().enumerate().take(from).rev().collect::<Vec<_>>()
        }
    }
}

pub trait FeatureExtractorOptions: IOOptions + Send + Sync {}

pub trait FeatureExtractor: Send + Sync {
    fn name(&self) -> String;

    fn filter<'ctx>(&self, target_type: bool) -> bool;

    fn extract(&self, slice_id: usize, slice: &Slice, trace: &Trace) -> serde_json::Value;
}

pub struct FeatureExtractors {
    extractors: Vec<Box<dyn FeatureExtractor>>,
}

impl FeatureExtractors {
    pub fn all() -> Self {
        Self {
            extractors: vec![
                Box::new(ArgumentPreconditionFeatureExtractor::new()),
                Box::new(ArgumentPostconditionFeatureExtractor::new()),
                Box::new(CausalityFeatureExtractor::new()),
                Box::new(ReturnValueFeatureExtractor::new()),
            ],
        }
    }

    pub fn extractors_for_target<'ctx>(has_return_type: bool) -> Self {
        Self {
            extractors: Self::all()
                .extractors
                .into_iter()
                .filter(|extractor| extractor.filter(has_return_type))
                .collect(),
        }
    }

    pub fn extract_features(&self, slice_id: usize, slice: &Slice, trace: &Trace) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        // Put the loc into feature file.
        map.insert(
            "loc".to_string(),
            serde_json::Value::String(trace.target_instr().loc.clone()),
        );
        // Acquire feature from each extractor.
        for extractor in &self.extractors {
            map.insert(extractor.name(), extractor.extract(slice_id, &slice, &trace));
        }
        serde_json::Value::Object(map)
    }

    pub fn load_trace(path: &PathBuf) -> Result<Trace, String> {
        load_json_t(path)
    }
}

pub trait FunctionTypesTrait<'ctx> {
    fn function_types(&self) -> HashMap<String, FunctionType<'ctx>>;
}

impl<'ctx> FunctionTypesTrait<'ctx> for Module<'ctx> {
    fn function_types(&self) -> HashMap<String, FunctionType<'ctx>> {
        let mut result = HashMap::new();
        for func in self.iter_functions() {
            result
                .entry(func.simp_name())
                .or_insert_with(|| func.get_function_type());
        }
        result
    }
}

pub struct FeatureExtractionContext<'a, 'ctx, O>
where
    O: FeatureExtractorOptions + IOOptions,
{
    pub options: &'a O,
    pub occurrences: HashMap<String, (bool, usize)>,
    pub func_types: HashMap<String, FunctionType<'ctx>>,
}

impl<'a, 'ctx, O> FeatureExtractionContext<'a, 'ctx, O>
where
    O: FeatureExtractorOptions + IOOptions,
{
    pub fn new(
        module: &'a Module<'ctx>,
        occurrences: HashMap<String, (bool, usize)>,
        options: &'a O,
    ) -> Result<Self, String> {
        let func_types = module.function_types();
        Ok(Self {
            options,
            occurrences,
            func_types,
        })
    }

    pub fn load_slices(&self, func: &String, num_slices: usize) -> Vec<Slice> {
        (0..num_slices)
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|slice_id| {
                let path = self.options.slice_target_file_path(func.as_str(), slice_id);
                load_json_t(&path).expect("Cannot load slice files")
            })
            .collect::<Vec<_>>()
    }

    pub fn load_trace_file_paths(&self, func: &String, slice_id: usize) -> Vec<(usize, PathBuf)> {
        match fs::read_dir(self.options.trace_target_slice_dir(func.as_str(), slice_id)) {
            Ok(paths) => paths
                .map(|path| {
                    let path = path.expect("Cannot read traces folder path").path();
                    let trace_id = path.file_stem().unwrap().to_str().unwrap().parse::<usize>().unwrap();
                    (trace_id, path)
                })
                .collect::<Vec<_>>(),
            _ => vec![],
        }
    }

    // Just for a specific bc-file(package)
    pub fn extract_features(&self, _: &mut LoggingContext) {
        fs::create_dir_all(self.options.feature_dir()).expect("Cannot create features directory");

        let occurs_pb_style = ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:50.green/white} {pos:>5}/{len:5} {percent}% {msg}")
            .progress_chars("##-");
        let occurs_pb = ProgressBar::new(self.occurrences.len() as u64).with_style(occurs_pb_style);
        occurs_pb.set_message("Total Functions");

        self.occurrences
            .par_iter()
            .progress_with(occurs_pb)
            .for_each(|(func, (has_return_type, num_slices))| {
                // Initialize extractors
                let extractors = FeatureExtractors::extractors_for_target(*has_return_type);

                // Load slices
                let slices = self.load_slices(&func, *num_slices);

                // Extract features
                slices.par_iter().enumerate().for_each(|(slice_id, slice)| {
                    // First create directory
                    fs::create_dir_all(self.options.feature_target_slice_dir(func.as_str(), slice_id))
                        .expect("Cannot create features func slice directory");

                    // Then load trace file directories
                    let traces = self.load_trace_file_paths(&func, slice_id);

                    // let style = ProgressStyle::default_bar()
                    // 	.template(
                    // 		"[{elapsed_precise}] {bar:40.cyan/white} Traces:{pos:>5}/{len:5} {msg}",
                    // 	)
                    // 	.progress_chars("##-");
                    // let pb = ProgressBar::new(traces.len() as u64).with_style(style);
                    // pb.set_message(&format!(
                    // 	"Traces of slice {}/{} for {}\r",
                    // 	slice_id,
                    // 	slices.len(),
                    // 	func
                    // ));

                    // iter for extracting features
                    traces
                        .into_par_iter()
                        // .progress_with(pb)
                        .for_each(|(trace_id, dir_entry)| {
                            // Load trace json
                            let trace = FeatureExtractors::load_trace(&dir_entry);

                            match trace {
                                Ok(trace) => {
                                    // Extract and dump features
                                    let features = extractors.extract_features(slice_id, &slice, &trace);
                                    let path =
                                        self.options
                                            .feature_target_slice_file_path(func.as_str(), slice_id, trace_id);
                                    dump_json(&features, path).expect("Cannot dump features json");
                                }
                                _ => {}
                            }
                        })
                });
            });
    }
}
