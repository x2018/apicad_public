#!/usr/bin/env python3
import os, time
import logging
from ..lib import utils, doc_utils
from . import preprocess_openssl, preprocess_linux, preprocess_libc, extract_semantic

this_path = os.path.dirname(os.path.realpath(__file__))
logger = logging.getLogger("doc-analyzer")


def setup_parser(subparsers):
    parser = subparsers.add_parser('doc-analyze', help="Analyze the document of API")
    parser.add_argument('--doc-dir', '-doc-dir', type=str, default=None,
                        help='The directory where to load doc, default is this_path/../doc_collector/doc_files')
    parser.add_argument('--outdir', '-outdir', type=str, default=None,
                        help='The directory where to output, default is this_path/doc_features')
    parser.add_argument('--target', '-target', type=str, default=None,
                        help='The target document. e.g. glibc/linux/openssl')
    parser.add_argument('--no-pre', '-no-pre', action='store_true', help='Skip preprocessing...')
    parser.add_argument('--no-sem', '-no-sem', action='store_true', help='Skip analyzing semantics...')
    parser.add_argument('--semantic-type', type=str, default=None, help='The type of semantics to be analyzed. Format: return|args|causality')
    parser.add_argument('--display', '-display', action='store_true', help='Display the intermediate handling results...')


targets = {
    'glibc': preprocess_libc.handle_glibc,  # For glibc
    'linux': preprocess_linux.handle_linux,  # For Linux
    'openssl': preprocess_openssl.handle_openssl,  # For openssl
}


def main(args):
    default_target = ["glibc", "linux", "openssl"]
    if args.doc_dir == None:
        args.doc_dir = os.path.join(this_path, "../doc_collector/doc_files")
    if utils.path_exist(args.doc_dir) != 0:
        return []
    if args.outdir == None:
        args.outdir = os.path.join(this_path, "./doc_features")
    if utils.mkdir(args.outdir) != 0:
        return []
    if args.target != None and args.target in default_target:
        default_target = [args.target]
    total_time_begin = time.time()
    func_list_file = os.path.join(args.outdir, "func_list.json")
    func_list = doc_utils.load_json(func_list_file)

    preprocess_dir = os.path.join(args.outdir, "preprocess")
    utils.mkdir(preprocess_dir)

    logger.info(utils.color_str("=== Starting to analyze ===", "green"))

    # 1. preprocessing the documentation files
    if not args.no_pre:
        for target in default_target:
            logger.info(f"Preprocessing documents for {target}")
            time_begin = time.time()
            func_list[target] = targets[target](args.doc_dir, preprocess_dir)
            time_end = time.time()
            logger.info(f"time consuming(s): {time_end - time_begin}")
        doc_utils.dump_json(func_list_file, func_list)

    # 2. extract semantics from the documentation
    if not args.no_sem:
        logger.info(f"Extracting semantics from preprocessed files")
        time_begin = time.time()
        raw_func_list = []
        for item in func_list:
            raw_func_list += func_list[item]
        extract_semantic.main(args.outdir, preprocess_dir, raw_func_list, args.display, args.semantic_type)
        time_end = time.time()
        logger.info(f"time consuming(s): {time_end - time_begin}")

    total_time_end = time.time()

    logger.info(utils.color_str(f"Total time consuming(s): {total_time_end - total_time_begin}", "green"))
    logger.info(utils.color_str("=== Finish analyze ===", "green"))
