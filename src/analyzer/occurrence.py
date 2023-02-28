import subprocess
import os, json
from ..lib import utils

this_path = os.path.dirname(os.path.realpath(__file__))

def setup_parser(subparsers):
    parser = subparsers.add_parser("occurrence", help="find functions occur in the codebase")
    parser.add_argument('--bc', type=str, default="", help='The .bc file to analyze')
    parser.add_argument('--target', "-target", type=str, default=None, help='The target func')
    parser.add_argument('--min-num', "--min", type=int, default=0, help='List the func which occurs >= min_num')
    parser.add_argument('--bcdir', '-bcdir', type=str, default=None,
                         help='The directory where to save/load .bc files, default is codebase/bc-files')
    parser.add_argument('--outdir','-outdir', type=str, default=None,
                         help='The directory where to save/load output, default is codebase/cad-output')

def run_occurrence(bc_file, args):
    print(utils.color_str(f"### Getting functions occur in {bc_file} to {args.outdir} ###", "green"))
    occurrence = "target/release/occurrence"
    occurrence_args = [bc_file, args.outdir]
    cmd = [occurrence] + occurrence_args
    ret = subprocess.run(cmd, cwd=this_path)
    if ret.returncode != 0:
        raise Exception(utils.color_str("Failure during run_occurrence"))

def occurrence_summary(dir_path):
    occurrence_dir = os.path.join(dir_path, "occurrences")
    occurrences = {}
    occurr_files = os.listdir(occurrence_dir)
    for file_name in occurr_files:
        file_path = os.path.join(occurrence_dir, file_name)
        if os.path.isfile(file_path) and file_name.endswith(".json"):
            with open(file_path, "r") as f:
                single_occurrence = json.load(f)
                for func in single_occurrence:
                    if func not in occurrences:
                        occurrences[func] = single_occurrence[func]
                    else:
                        occurrences[func] += single_occurrence[func]
    with open(os.path.join(dir_path, "total_occurrences.json"), "w") as f:
        json.dump(occurrences, f)

def main(args):
    # Acquire the .bc files to analyze
    bc_files_to_run = utils.bc_files_to_run(args)
    if args.target == None:
        if len(bc_files_to_run) > 0:
            for bc_file in bc_files_to_run:
                try:
                    run_occurrence(bc_file, args)
                except Exception as e:
                    print(e)
            occurrence_summary(args.outdir)
        else:
            raise Exception(utils.color_str(f"no .bc files in {args.bcdir}"))
    else:
        occurrences_file = os.path.join(args.outdir, "total_occurrences.json")
        if os.path.isfile(occurrences_file):
            with open(occurrences_file, "r") as f:
                occurrences = json.load(f)
                target_is_exist = False
                for func in occurrences:
                    if args.target not in func:
                        continue
                    target_is_exist = True
                    occurrences_time = occurrences[func]
                    if occurrences_time > args.min_num:
                        print(f"the occurrences time of {func} is: {occurrences_time}")
                    else:
                        continue
                if not target_is_exist:
                    print(f"Cannot find {args.target} in {occurrences_file}")
        else:
            print(f"Cannot find {occurrences_file}. ")
