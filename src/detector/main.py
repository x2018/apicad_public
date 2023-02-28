#!/usr/bin/env python3
import os
import logging
from ..lib import utils
from .detector import Detector
from .features_handler import Code_feature_hanlder, Doc_feature_handler

this_path = os.path.dirname(os.path.realpath(__file__))
logger = logging.getLogger("detection")


def setup_parser(subparsers):
    parser = subparsers.add_parser('detect', help="Detect API misuse bugs")
    parser.add_argument('--feature-dir', type=str, default=None,
                        help='The feature_type directory to analyze, default is codebase/cad-output/features')
    parser.add_argument('--doc-dir', '-doc-dir', type=str, default=None,
                        help='The directory where to load doc-features, default is this_path/../doc_analyzer/doc_features')
    parser.add_argument('--outdir', '-outdir', type=str, default=None,
                        help='The directory where to save/load output, default is codebase/cad-output')
    parser.add_argument('--target-fn', '-target', type=str, help='Target function(s) to detect')
    parser.add_argument('--type', '-type',
                        help='Choose for a special check, e.g. \'retval\', \'arg.pre\', \'arg.post\', \'causality\'')
    parser.add_argument('--threshold', type=float, default=None, help="The threshold of frequency-based specification.")
    parser.add_argument('--rho', type=int, default=None, help="Set the hyper parameter rho of the threshold.")
    parser.add_argument('--rm-dup', action='store_true', help="Count the same feature at a location only once.")
    parser.add_argument('--display-spec', action='store_true')
    parser.add_argument('--enable-doc', action='store_true')
    parser.add_argument('--disable-code', action='store_true', help="Only enable documents.")
    parser.add_argument('--only-report-locations', action='store_true')


def main(args):
    doc_handler = None
    code_feature_paths = utils.features_to_analyze(args)
    if args.enable_doc or args.disable_code:
        if args.doc_dir == None:
            args.doc_dir = os.path.join(this_path, "../doc_analyzer/doc_features")
        doc_feature_path = os.path.join(args.doc_dir, "doc_feature.json")
        if utils.path_exist(doc_feature_path) == 0:
            doc_handler = Doc_feature_handler(doc_feature_path, args.display_spec)

    logger.info(f"Detecting for {len(code_feature_paths)} functions")

    code_handler = Code_feature_hanlder(args.rm_dup, args.display_spec, args.threshold, args.rho, args.disable_code)

    log_path = os.path.join(args.outdir, "bugreport.txt")
    detect_instance = Detector(code_handler,
                               doc_handler,
                               log_path,
                               code_feature_paths,
                               check_type=args.type)
    detect_instance.detect(args.only_report_locations)

    logger.info(f"Have dumpped results to {log_path} ###")
