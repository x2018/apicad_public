#!/usr/bin/env python3
import os, subprocess, logging
from ..lib import utils

logger = logging.getLogger("generate-bc")


def setup_parser(subparsers):
    parser = subparsers.add_parser("generate-bc", help="generate .bc files for a complied codebase")
    parser.add_argument('--bcdir',
                        "-bcdir",
                        type=str,
                        default=None,
                        help='The directory where to save/load .bc files, default is codebase/bc-files')
    parser.add_argument('--target', "-target", type=str, default=None, help='The target file')
    parser.add_argument('--linux', "-linux", action='store_true', help='For linux')
    parser.add_argument('--obj', "-obj", action='store_true', help='For *.o')


def search_libs(path, linux=False):
    libs = []
    if os.path.exists(path):
        if linux == False:
            # ignore .la
            out = subprocess.run(['find', path, '-name', 'lib*.so*', '-o', '-name', '*.a'], stdout=subprocess.PIPE)
            if len(out.stdout.splitlines()) == 0:
                out = subprocess.run(['find', path, '-name', '*.so*', '-o', '-name', '*.a'], stdout=subprocess.PIPE)
            filterd_paths = [line.decode('utf-8') for line in out.stdout.splitlines() if not os.path.islink(line)]
            tmp_libs = []
            for l in filterd_paths:
                if not l.endswith("bc") and not l.endswith("json"):
                    # In each folder, only stay one file with same basename
                    tmp_l = os.path.join(os.path.dirname(l), os.path.basename(l).split('.')[0])
                    if tmp_l not in tmp_libs:
                        tmp_libs.append(tmp_l)
                        libs.append(l)
        else:
            root_dirs = os.listdir(path)
            for name in root_dirs:
                if not os.path.isdir(name):
                    root_dirs.remove(name)
            for root_dir in root_dirs:
                # In each top tier folder and its subs, we only stay one file with same basename.
                tmp_libs = []
                for root, dirs, files in os.walk(root_dir):
                    for name in files:
                        lib = os.path.join(root, name)
                        if lib.endswith(".a") and name not in tmp_libs:
                            tmp_libs.append(name)
                            libs.append(lib)
    return libs


def search_doto(path):
    if os.path.exists(path):
        # ignore .lo .ko?
        out = subprocess.run(['find', path, '-name', '*.o'], stdout=subprocess.PIPE)
        return [line.decode('utf-8') for line in out.stdout.splitlines() if not os.path.islink(line)]
    else:
        return []


def collect_targets(args):
    logger.info(utils.color_str(f"=== Generating .bc files to {args.bcdir} ===", "green"))
    if "target" in args and args.target != None:
        out = subprocess.run(['find', args.codebase, '-name', args.target], stdout=subprocess.PIPE)
        targets = [line.decode('utf-8') for line in out.stdout.splitlines() if not os.path.islink(line)]
    elif "linux" in args and args.linux != False:
        targets = search_libs(args.codebase, True)
        if len(targets) == 0:
            raise Exception(utils.color_str("Can not find built-in.a"))
    else:
        targets = search_libs(args.codebase)
        if ("obj" in args and args.obj) or len(targets) == 0:
            targets = search_doto(args.codebase)
            if len(targets) == 0:
                raise Exception(utils.color_str("Can not find libs or .o in current codebase"))

    return targets


def generate_bc(bcdir, targets):
    duplicate_name = {}
    for file in targets:
        name = os.path.basename(file)
        if name not in duplicate_name:
            duplicate_name[name] = 0
        else:
            duplicate_name[name] += 1
        name_id = duplicate_name[name]
        ret = None
        if not os.path.exists(f"{bcdir}/{os.path.basename(file)}_{name_id}.bc"):
            if file[-2:] == '.a':
                ret = subprocess.run(["a2bc", file, str(name_id), bcdir], stderr=subprocess.STDOUT)
            else:
                ret = subprocess.run(["extract-bc", file, "--output", f"{bcdir}/{os.path.basename(file)}_{name_id}.bc"],
                                     stderr=subprocess.STDOUT)
        else:
            logger.info(utils.color_str(f"=== skip {file}, id = {name_id} ===", "green"))
        if ret and ret.returncode != 0:
            raise Exception(utils.color_str("Failure during generating bc"))


def main(args):
    if args.bcdir == None:
        args.bcdir = os.path.join(args.codebase, "bc-files")
    if utils.mkdir(args.bcdir) != 0:
        return -1

    target_files = collect_targets(args)
    generate_bc(args.bcdir, target_files)
    return 0
