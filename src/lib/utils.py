#!/usr/bin/env python3
import os
import shutil
import logging

from ..builder import generate_bc


def color_str(str, color='red'):
    colors = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan']
    if color not in colors:
        return str
    return "\033[3%dm" % colors.index(color) + str + "\033[m"


logging.basicConfig(level=logging.INFO,
                    format='[%(asctime)s] %(levelname)s(%(name)s): %(message)s',
                    datefmt='%Y-%m-%d %H:%M:%S')
logger = logging.getLogger(color_str(f"UTILS", 'yellow'))


def config_log_file(logger, log_path, mode="w"):
    fh = logging.FileHandler(log_path, mode)
    f_format = logging.Formatter("[%(asctime)s] %(levelname)s: %(message)s", datefmt='%Y-%m-%d %H:%M:%S')
    fh.setFormatter(f_format)
    logger.addHandler(fh)


def rmdir(path: str):
    if os.path.isdir(path) and not os.path.islink(path):
        shutil.rmtree(path)
    elif os.path.exists(path):
        os.remove(path)


def mkdir(path: str):
    if not os.path.exists(path):
        try:
            os.mkdir(path)
        except PermissionError:
            logger.warn(color_str(f"Permission denied: {path}"))
            return -1
        except:
            logger.warn(color_str(f"can not create the directory: {path}"))
            return -1
    return 0


def generate_tmp_folder(base_dir="."):
    # xxx: Maybe need to generate randomly if multiple instances are running at the same time?
    tmp_name = "tmp_folder"
    if mkdir(base_dir + "/" + tmp_name) == 0:
        return tmp_name
    return ""


def path_exist(path: str, no_warn=False):
    if not os.path.exists(path):
        if not no_warn:
            logger.warn(color_str(f"can not find the file or directory: {path}"))
        return -1
    return 0


def read_file(file_path: str):
    data = ""
    with open(file_path) as f:
        data = f.read()
    return data


def get_bc_files(out_dir: str):
    for root, _, files in os.walk(out_dir):
        for name in files:
            bc_file = os.path.join(root, name)
            if bc_file.endswith(".bc"):
                yield bc_file


def get_all_bc_files(in_dir: str):
    files = []
    for file in get_bc_files(in_dir):
        files.append(file)
    return files


def bc_files_to_run(args):
    if args.outdir == None:
        args.outdir = os.path.join(args.codebase, "cad-output")
    else:
        args.outdir = os.path.abspath(args.outdir)
    if mkdir(args.outdir) != 0:
        return []

    if args.bcdir == None:
        args.bcdir = os.path.join(args.codebase, "bc-files")
    else:
        args.bcdir = os.path.abspath(args.bcdir)
    if path_exist(args.bcdir) != 0 and generate_bc.main(args) != 0:
        return []

    return [bc_file for bc_file in get_all_bc_files(args.bcdir) if args.bc == '' or args.bc in bc_file]


def get_feature_file(out_dir: str):
    for root, _, files in os.walk(out_dir):
        for name in files:
            feature_file = os.path.join(root, name)
            if feature_file.endswith(".fea.json"):
                yield feature_file


def get_all_feature_files(in_dir: str, target_fn=None):
    result = {}
    functions = os.listdir(in_dir)
    files = []
    for func in functions:
        if target_fn == None or target_fn == func: # target_fn in func:
            files = get_feature_file(os.path.join(in_dir, func))
            result[func] = files
    return result


'''
{"func_name": [feature_paths]... }
'''
def features_to_analyze(args):
    if args.outdir == None:
        args.outdir = os.path.join(args.codebase, "cad-output")
    else:
        args.outdir = os.path.abspath(args.outdir)
    if mkdir(args.outdir) != 0:
        return []

    if args.feature_dir == None:
        args.feature_dir = os.path.join(args.outdir, "features")
    else:
        args.feature_dir = os.path.abspath(args.feature_dir)
    if path_exist(args.feature_dir) != 0:
        return {}

    return get_all_feature_files(args.feature_dir, args.target_fn)
