#!/usr/bin/env python3
from argparse import ArgumentParser
import os

from .lib import utils
from . import detector
from . import analyzer
from . import doc_collector
from . import doc_analyzer
from . import builder

modules = {
    'build': builder.build,
    'generate-bc': builder.generate_bc,
    'analyze': analyzer.analyzer,
    'doc-collect': doc_collector.doc_collector,
    'doc-analyze': doc_analyzer.doc_analyzer,
    'occurrence': analyzer.occurrence,
    'detect': detector.main,
}


def arg_parser():
    parser = ArgumentParser()
    parser.add_argument('-codebase', type=str, default=None, help='the codebase directory, default is ./')
    subparsers = parser.add_subparsers(dest="cmd")
    subparsers.required = True

    for (_, module) in modules.items():
        module.setup_parser(subparsers)
    return parser


def main():
    # parse the arguments
    parser = arg_parser()
    args = parser.parse_args()

    if args.codebase == None:
        args.codebase = os.getcwd()
    else:
        args.codebase = os.path.abspath(args.codebase)

    if utils.path_exist(args.codebase) != 0:
        return

    # Execute the main function of that module
    if args.cmd:
        modules[args.cmd].main(args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
