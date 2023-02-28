#!/usr/bin/env python3
import os, subprocess
import argparse


def setup_parser(subparsers):
    parser = subparsers.add_parser("build", help="make a compiled codebase")
    parser.add_argument("cmds",
                        type=str,
                        nargs=argparse.REMAINDER,
                        default=[],
                        help="the conmand of configure or make etc")


def generate_default_Makefile():
    content = "# Makefile for testcase\n\n"
    content += "SRCS = $(wildcard *.c)\n"
    content += "OBJS = $(SRCS:.c=.o)\n\n"
    content += "%o: %c\n"
    content += "\t$(CC) $(CFLAGS) -c -o $@ $<\n\n"
    content += "all: $(OBJS)\n\n"
    content += "clean: \n"
    content += "\trm -rf $(OBJS) \n\n"
    content += ".PHONY: all clean"
    path = os.path.join(os.getcwd(), "Makefile")
    print(f"### Generating the default Makefile to {path} ###")
    with open(path, "w") as f:
        f.write(content)


def compile_env():
    env = os.environ.copy()
    env["LLVM_COMPILER"] = "clang"
    env["CC"] = "wllvm"
    # env["HOSTCC"] = "wllvm"
    env["CXX"] = "wllvm++"
    env["CFLAGS"] = "-g -O0"
    # "KBUILD_HOSTCFLAGS" "KBUILD_CFLAGS" "KBUILD_USERCFLAGS" "KBUILD_USERCXXFLAGS"
    env["CXXFLAGS"] = "-g -O0"
    env["CPPFLAGS"] = "-g -O0"
    return env


def build(args):
    cmds = args.cmds
    if cmds[0] == "make" and not os.path.exists("Makefile"):
        generate_default_Makefile()
    env = compile_env()
    ret = subprocess.run(cmds, stderr=subprocess.STDOUT, env=env)
    if ret.returncode != 0:
        raise Exception(f"{cmds} failed")


def main(args):
    if args.cmds:
        build(args)
    else:
        print("lack the conmand such as configure or make etc")
