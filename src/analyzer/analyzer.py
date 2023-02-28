#!/usr/bin/env python3
import subprocess
import os
import json
import logging
import multiprocessing as mp

from ..lib import utils

logger = logging.getLogger("Analyzer")

this_path = os.path.dirname(os.path.realpath(__file__))

analyzer = "target/release/analyzer"

def setup_parser(subparsers):
    parser = subparsers.add_parser("analyze", help="analyze .bc files to generate symbolic traces and features")
    parser.add_argument('--print-options', action='store_true')
    parser.add_argument('--print-call-graph', action='store_true')
    parser.add_argument('--serial', '-s', action='store_true', help='Scheduling internal jobs in serial')
    parser.add_argument('--serial-bc', action='store_true', help='Analyze on bc files one by one')
    parser.add_argument('--not-random', action='store_true', help="Do not randomly schedule the execution work")
    parser.add_argument('--slice-depth', type=int, default=0, help='Slice depth')
    parser.add_argument('--max-timeout', '-timeout', type=int, default=5, help="max timeout(s) for one work")
    parser.add_argument('--use-batch', action='store_true', help="Do use batch")
    parser.add_argument('--batch-size', type=int)
    parser.add_argument('--trace-only', action='store_true', help='Only generate symolic traces')
    parser.add_argument('--feature-only', action='store_true', help='Directly generate feature')
    parser.add_argument('--regex', action='store_true')
    parser.add_argument('--target-fn', '-target', default=[], type=str, nargs='+',
                         help='Target function(s) to be analyzed')
    parser.add_argument('--exclude-fn', '-exclude', default=[], type=str, nargs='+',
                         help='Function(s) to be ignored')
    parser.add_argument('--max-trace-per-slice', type=int, default=50)
    parser.add_argument('--max-explored-trace-per-slice', type=int, default=1000)
    parser.add_argument('--max-node-per-trace', type=int, default=2000)
    parser.add_argument('--step-in-anytime', action='store_true',
                         help='Step into the calls even if the slice depth is zero')
    parser.add_argument('--rough-mode', action='store_true', help='Save traces without satisfying path constraints')
    parser.add_argument('--bc', "-bc", type=str, default="", help='The .bc file to analyze')
    parser.add_argument('--bcdir', '-bcdir', type=str, default=None,
                         help='The directory where to save/load .bc files, default is codebase/bc-files')
    parser.add_argument('--outdir','-outdir', type=str, default=None,
                         help='The directory where to save/load output, default is codebase/cad-output')

def get_analyzer_args(bc_file, args, tmp_folder=None, extract_features=False):
    bc_name = os.path.basename(bc_file)
    base_args = [bc_file, args.outdir, '--subfolder', bc_name, '--slice-depth', str(args.slice_depth)]

    if args.print_call_graph:
        base_args += ['--print-call-graph']

    if args.print_options:
        base_args += ['--print-options']

    if args.serial:
        base_args += ['--use-serial']

    if args.not_random:
        base_args += ['--not-random-scheduling']

    if args.max_timeout:
        base_args += ['--max-timeout', str(args.max_timeout)]

    if args.use_batch:
        base_args += ['--use-batch']

    if args.batch_size:
        base_args += ['--batch-size', str(args.batch_size)]

    if args.feature_only:
        base_args += ['--feature-only']

    if args.regex:
        base_args += ['--use-regex-filter']

    if args.target_fn:
        base_args += ['--target-inclusion-filter'] + args.target_fn

    if args.exclude_fn:
        base_args += ['--target-exclusion-filter'] + args.exclude_fn

    if args.max_trace_per_slice != None:
        base_args += ['--max-trace-per-slice', str(args.max_trace_per_slice)]

    if args.max_explored_trace_per_slice != None:
        base_args += ['--max-explored-trace-per-slice', str(args.max_explored_trace_per_slice)]

    if args.max_node_per_trace != None:
        base_args += ['--max-node-per-trace', str(args.max_node_per_trace)]

    if extract_features == False:
        base_args += ['--no-feature']

    if args.step_in_anytime:
        base_args += ['--step-in-anytime']

    if args.rough_mode:
        base_args += ['--rough-mode']

    if tmp_folder != None:
        base_args += ['--metadata-file', f'{tmp_folder}/{bc_name}_metadata.json']
        base_args += ['--target-num-slices-map-file', f'{tmp_folder}/{bc_name}.json']

    return base_args

def run_analyzer_on_bc_file(bc_file, args, tmp_folder=None, only_one=False):
    logger.info(utils.color_str(f"=== Running analyzer on {bc_file}... ===", "green"))
    if only_one:
        analyzer_args = get_analyzer_args(bc_file, args, extract_features=True)
    else:
        analyzer_args = get_analyzer_args(bc_file, args, tmp_folder=tmp_folder)

    cmd = [analyzer] + analyzer_args
    ret = subprocess.run(cmd, cwd=this_path)
    if ret.returncode != 0:
        # This should rarely happen.
        # In fact, there may be other more complex reasons if it happens.
        alert_text = utils.color_str(f"Failure during handling {os.path.basename(bc_file)}.\n" \
                        f"error code: {ret.returncode}.\n" \
                        "Take command perror/errno to see the description or check it in <errno.h>.\n" \
                        "Note: The most common reason is killed by OS because of the limited resource.\n" \
                        "\tUsing -s can relieve some pressure.")
        logger.info(alert_text)
        return ""

    if only_one:
        return
    bc_name = os.path.basename(bc_file)
    output_file = args.outdir + f'/{tmp_folder}/{bc_name}.json'
    # Format: {"func_name": (has_return_type, slices_num)}
    with open(output_file) as f:
        return json.load(f)

def par_job(inputs):
    return run_analyzer_on_bc_file(inputs[0], inputs[1], inputs[2])

def combine_metadata(outdir, bc_files, tmp_folder):
    combined_metadata = {}
    for bc_file in bc_files:
        bc_name = os.path.basename(bc_file)
        output_file = outdir + f'/{tmp_folder}/{bc_name}_metadata.json'
        new_metadata = {}
        try:
            with open(output_file) as f:
                new_metadata = json.load(f)
            if combined_metadata == {}:
                combined_metadata = new_metadata
            else:
                for key in combined_metadata:
                    combined_metadata[key] += new_metadata[key]
        except:
            continue
    return combined_metadata

def generate_func_num_slices_map(output_dir, package_occurs_map, tmp_folder):
    functions = {}
    for bc_path, occurrences in package_occurs_map.items():
        bc_name = os.path.basename(bc_path)
        for func_name, (has_return_type, num_slices) in occurrences.items():
            if num_slices > 0:
                if not func_name in functions:
                    functions[func_name] = {
                        "name": func_name,
                        "package_num_slices": [],
                        "has_return_type": has_return_type
                    }
                functions[func_name]["package_num_slices"].append([bc_name, num_slices])
    ''' 
    func_num_slices_map: {
        "functions": [{"name": "function_name", 
            "has_return_type": true/false,
            "package_num_slices": [["bcfile_name_1", num_slices_1], ...]}, ...]
    }
    '''
    func_num_slices_map = {"functions": list(functions.values())}

    filename = output_dir + "/" + tmp_folder + "/ALL.json"
    with open(filename, "w") as f:
        json.dump(func_num_slices_map, f)

    return filename

def run_feature_extractor(func_num_slices_map, args):
    logger.info(utils.color_str("=== Running feature extraction... ===", "green"))
    extractor = "target/release/feature-extract"
    extractor_args = [func_num_slices_map, args.outdir]
    cmd = [extractor] + extractor_args
    ret = subprocess.run(cmd, cwd=this_path, stderr=subprocess.STDOUT)
    if ret.returncode != 0:
        logger.warn(utils.color_str("Failure during run_feature_extractor"))

def main(args):
    # Acquire the .bc files to analyze
    bc_files_to_run = utils.bc_files_to_run(args)
    log_path = os.path.join(args.outdir, "analyze_log.txt")
    utils.config_log_file(logger, log_path, "a")

    logger.info(utils.color_str("=== Starting to analyze ===", "green"))
    if len(bc_files_to_run) > 0:
        if len(bc_files_to_run) > 1 or args.trace_only:
            # Generate temporary folder that is shared among all runs
            tmp_folder_name = utils.generate_tmp_folder(args.outdir)
            if tmp_folder_name == "":
                logger.warn(utils.color_str(f"Failure during making temp directory"))
            # {"bc_file_path": {"func_name": (has_return_type, slices_num)}}
            package_occurs_map = {}

            # Run analyzer on each .bc file
            if args.serial_bc:
                for bc_file in bc_files_to_run:
                    try:
                        occurrences = run_analyzer_on_bc_file(bc_file, args, tmp_folder_name)
                        if len(occurrences) > 0:
                            package_occurs_map[bc_file] = occurrences
                    except Exception as e:
                        logger.info(e)
                        exit()
            else:
                import signal
                ignore_sigint = signal.signal(signal.SIGINT, signal.SIG_IGN)
                pool = mp.Pool(processes=mp.cpu_count(), )
                par_job_inputs = [(bc_file, args, tmp_folder_name) for bc_file in bc_files_to_run]
                try:
                    signal.signal(signal.SIGINT, ignore_sigint) # re-enable to catch sigint
                    results = pool.map_async(par_job, par_job_inputs)
                    pool.close()
                    pool.join()
                    results = results.get()
                    for i, bc_file in enumerate(bc_files_to_run):
                        if len(results[i]) > 0:
                            package_occurs_map[bc_file] = results[i]
                except KeyboardInterrupt:
                    logger.warn(utils.color_str(f"=== KeyboardInterrupt: Self-Terminated ===\n"))
                    # Kill itself and its subprocesses
                    os.killpg(os.getpid(), signal.SIGKILL)

            # Make statistics and output the total metadata in the original order
            metadata_ordered_item = ["proper_trace_count", "path_unsat_trace_count", "branch_explored_trace_count",
                "duplicate_trace_count", "no_target_trace_count", "exceeding_length_trace_count",
                "timeout_trace_count", "unreachable_trace_count", "explored_trace_count"]
            total_metadata = combine_metadata(args.outdir, bc_files_to_run, tmp_folder_name)
            ordered_metadata = dict()
            for item in metadata_ordered_item:
                ordered_metadata[item] = total_metadata[item]
            logger.info(utils.color_str("Total Metadata: " + str(ordered_metadata), "cyan"))

            if not args.trace_only:
                # Run feature extractor on all data generated
                func_num_slices_map = generate_func_num_slices_map(args.outdir, package_occurs_map, tmp_folder_name)
                run_feature_extractor(func_num_slices_map, args)

            # Remove the tmp folder
            utils.rmdir(args.outdir + "/" + tmp_folder_name)

        else: # There is only one bc file to be analyzed
            bc_file = bc_files_to_run[0]
            run_analyzer_on_bc_file(bc_file, args, only_one=True)
        logger.info(utils.color_str("=== Finished ===\n", "green"))
    else:
        logger.warn(utils.color_str(f"no .bc files in {args.bcdir}\n"))
