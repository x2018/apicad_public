#!/usr/bin/env python3
import logging, os, time
from . import collect_libc, collect_linux, collect_openssl
from ..lib import utils

logger = logging.getLogger("doc-collector")
this_path = os.path.dirname(os.path.realpath(__file__))


def setup_parser(subparsers):
    parser = subparsers.add_parser('doc-collect', help="Collect the API document from website")
    parser.add_argument('--doc-dir', '-doc-dir', type=str, default=None,
                        help='The directory where to save doc, default is this_path/doc-files')
    parser.add_argument('--target', '-target', type=str, default=None,
                        help='The target document. e.g. glibc/linux/openssl')


'''
documentation source:
    https://www.gnu.org/software/libc/
    https://www.kernel.org/doc/man-pages/
    https://www.openssl.org/docs/manmaster/man3/
'''

targets = {
    'glibc': collect_libc.handle_glibc,  # For glibc
    'linux': collect_linux.handle_linux,  # For Linux
    'openssl': collect_openssl.handle_openssl,  # For openssl
}


def main(args):
    default_target = ["glibc", "linux", "openssl"]
    if args.doc_dir == None:
        args.doc_dir = os.path.join(this_path, "doc_files")
    if utils.mkdir(args.doc_dir) != 0:
        return []
    if args.target != None and args.target in default_target:
        default_target = [args.target]
    total_time_begin = time.time()
    logger.info(utils.color_str("=== Starting to collect ===", "green"))
    for target in default_target:
        logger.info(f"Collecting documents for {target}")
        time_begin = time.time()
        targets[target](args.doc_dir)
        time_end = time.time()
        logger.info(f"time consuming(s): {time_end - time_begin}")
    total_time_end = time.time()
    logger.info(utils.color_str(f"Total time consuming(s): {total_time_end - total_time_begin}", "green"))
    logger.info(utils.color_str("=== Finish collect ===", "green"))
