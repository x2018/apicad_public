#!/usr/bin/env python3

VAR_ARG_KEYWORD = ['print', 'execl'] # xxx: cannot be handled such as with variable length argument?
OTHER_KEYWORD = ["fetch", "insert", "push", "pop", "read", "write", "encode", "decode"]
PRE_KEYWORD = ["alloc", "new", "clone", "create", "dup", "init", "open", "_lock"]
POST_KEYWORD = ["free", "release", "clear", "destroy", "clean", "close", "_unlock"]
PRESEQUENCE_KEYWORD = PRE_KEYWORD + OTHER_KEYWORD
SUBSEQUENT_KEYWORD = POST_KEYWORD + OTHER_KEYWORD


def has_keyword(name, keywords):
    for keyword in keywords:
        if keyword in name.lower():
            return True
    return False


def is_variable_argument(name):
    for keyword in VAR_ARG_KEYWORD:
        length = len(keyword)
        if keyword == name[:length].lower():
            return True
    return False


def is_pre(name):
    return has_keyword(name, PRE_KEYWORD)


def is_pre_seq(name):
    return has_keyword(name, PRESEQUENCE_KEYWORD)


def is_post(name):
    return has_keyword(name, POST_KEYWORD)


def is_subsequent(name):
    return has_keyword(name, SUBSEQUENT_KEYWORD)
