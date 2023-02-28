use indicatif::*;
use llir::types::*;
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use structopt::StructOpt;

use analyzer::{feature_extraction::*, options::*, utils::*};

#[derive(StructOpt, Debug)]
#[structopt(name = "feature-extract")]
pub struct Options {
    #[structopt(index = 1, required = true, value_name = "INPUT")]
    input: String,

    #[structopt(index = 2, required = true, value_name = "OUTPUT")]
    output: String,
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

// Input file
// func_num_slices_map:
// {
//     "functions": [{"name": "function_name",
//                    "has_return_type": true/false,
//                    "package_num_slices": [["bcfile_name_1", num_slices_1], ...]}, ...]
// }

#[derive(Deserialize)]
pub struct InputFunction {
    pub name: String,

    pub has_return_type: bool,

    // (.bc file name, number of slices)
    pub package_num_slices: Vec<(String, usize)>,
}

#[derive(Deserialize)]
pub struct Input {
    pub functions: Vec<InputFunction>,
}

impl Input {
    pub fn from_options(options: &Options) -> Self {
        load_json_t(&options.input_path()).expect("Cannot load input")
    }
}

pub type TargetPackageNumSlicesMap = HashMap<String, (bool, Vec<(String, usize)>)>;

pub type Packages<'ctx> = HashMap<String, HashMap<String, FunctionType<'ctx>>>;

fn load_slices(options: &Options, func: &str, package: &str, num_slices: usize) -> Vec<Slice> {
    (0..num_slices)
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|slice_id| {
            let path = options.slice_target_package_file_path(func, package, slice_id);
            load_json_t(&path).expect("Cannot load slice file")
        })
        .collect::<Vec<_>>()
}

fn load_trace_file_paths(options: &Options, func: &str, package: &str, slice_id: usize) -> Vec<(usize, PathBuf)> {
    fs::read_dir(options.trace_target_package_slice_dir(func, package, slice_id))
        .expect("Cannot read traces folder")
        .map(|path| {
            let path = path.expect("Cannot read traces folder path").path();
            let trace_id = path.file_stem().unwrap().to_str().unwrap().parse::<usize>().unwrap();
            (trace_id, path)
        })
        .collect::<Vec<_>>()
}

fn main() -> Result<(), String> {
    let options = Options::from_args();
    let input = Input::from_options(&options);

    println!("Building target functions map...");

    let mut func_num_slices_map = TargetPackageNumSlicesMap::new();
    for input_function in input.functions {
        func_num_slices_map.insert(
            input_function.name,
            (input_function.has_return_type, input_function.package_num_slices),
        );
    }

    let func_map_style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:50.green/white} {pos:>5}/{len:5} {percent}% {msg}")
        .progress_chars("##-");
    let func_map_pb = ProgressBar::new(func_num_slices_map.len() as u64).with_style(func_map_style);
    func_map_pb.set_message("Total Functions");

    func_num_slices_map.into_par_iter().progress_with(func_map_pb).for_each(
        |(func, (has_return_type, package_num_slices))| {
            let extractors = FeatureExtractors::extractors_for_target(has_return_type);

            package_num_slices.into_par_iter().for_each(|(package, num_slices)| {
                let slices = load_slices(&options, &func, &package, num_slices);
                slices.par_iter().enumerate().for_each(|(slice_id, slice)| {
                    // First create directory
                    fs::create_dir_all(options.feature_target_package_slice_dir(&func, &package, slice_id))
                        .expect("Cannot create features func slice directory");

                    // Then load trace file directories
                    let traces = load_trace_file_paths(&options, &func, &package, slice_id);

                    // let style = ProgressStyle::default_bar()
                    // 	.template("[{elapsed_precise}] {bar:40.cyan/white} Traces:{pos:>5}/{len:5} {msg}")
                    // 	.progress_chars("##-");
                    // let pb = ProgressBar::new(traces.len() as u64).with_style(style);
                    // pb.set_message(&format!("Traces of slice {}/{} for {}\r", slice_id, slices.len(), func));

                    // Iter for extracting features
                    traces
                        .into_par_iter()
                        // .progress_with(pb)
                        .for_each(|(trace_id, dir_entry)| {
                            let trace = FeatureExtractors::load_trace(&dir_entry);

                            match trace {
                                Ok(trace) => {
                                    // Extract and dump features
                                    let features = extractors.extract_features(slice_id, &slice, &trace);
                                    let path = options
                                        .feature_target_package_slice_file_path(&func, &package, slice_id, trace_id);
                                    dump_json(&features, path).expect("Cannot dump features json");
                                }
                                _ => {}
                            }
                        });
                })
            })
        },
    );

    Ok(())
}
