#!/usr/bin/env python3
from . import utils
import re, json


def rm_useless_asterisk(string: str):
    if len(string) == 0:
        return ""
    head_index = 0
    end_index = len(string)
    while head_index < len(string) and string[head_index] == '*':
        head_index += 1
    while end_index > 0 and string[end_index - 1] in ['*', '.', '\n']:
        end_index -= 1
    return string[head_index:end_index]


def rm_useless_space(string: str, head=True, end=True):
    if len(string) == 0:
        return ""
    head_index = 0
    end_index = len(string)
    if head:
        while head_index < len(string) and string[head_index] in [' ', '\n']:
            head_index += 1
    if end:
        while end_index > 0 and string[end_index - 1] in [' ', '\n']:
            end_index -= 1
    return string[head_index:end_index]


def rm_annotations(ori_def):
    new_def = re.sub(r'/\*.*?\*/', '', ori_def, flags=re.S)
    new_def = re.sub(r'(//.*)', '', new_def)
    return new_def


'''
Get the definition of the function.
Note: this is an empirical implementation.
Output:
    outdated: [type, func_name, arg1, arg2, ..., argn]
    now: {"func_name": .., "func_type": .., "args_name": .., "args_type": ..}
xxx: should we remain the type of arguments?
TODO: further identify the sub-arguments of internal callback functions?
'''
def get_definition(definition):
    # output
    func_name, func_type = "", ""
    args_name, args_type = [], []
    arg_name_tmp, arg_type_tmp = "", ""
    # status
    def_frame_size = 0
    arg_status, fun_name_status = False, False
    valid_position = False
    is_callback_arg = False
    # Remove annotations
    definition = rm_annotations(definition)
    # From the last character to the first
    for i, c in enumerate(definition[::-1]):
        if is_callback_arg:
            arg_name_tmp += c
        # name/parameter can be combined with "A-Z", "a-z", "0-9" or "_".
        elif arg_status == True or fun_name_status == True:
            if (c >= 'a' and c <= 'z') or (c >= 'A' and c <= 'Z') or (c >= '0' and c <= '9') \
                    or c == '_' or c == '[' or c == ']':
                if arg_status:
                    arg_name_tmp += c
                else:
                    func_name += c
                valid_position = True
            elif valid_position == False and c == ' ':  # skip invalid space
                continue
            else:
                if fun_name_status == True:  # store the type and return
                    fun_name_status = False
                    func_name = func_name[::-1]
                    func_type = definition[:-i]
                    break
                arg_name_tmp = rm_useless_space(arg_name_tmp[::-1])
                if arg_name_tmp not in ["", "void"]:
                    args_name.append(arg_name_tmp)
                # Reset status...
                arg_status = False  # reset arg_status
                arg_name_tmp = ""  # clear up arg_name_tmp
                valid_position = False  # reset valid_position
        # In the current function
        if def_frame_size == 1:
            if c == ',':
                if arg_name_tmp != "":
                    arg_name_tmp = rm_useless_space(arg_name_tmp[::-1])
                    if arg_name_tmp not in ["", "void"]:
                        args_name.append(arg_name_tmp)
                    arg_name_tmp = ""
                arg_status = True
                arg_type_tmp = rm_useless_space(arg_type_tmp[::-1])
                if arg_type_tmp == "" and len(args_type) == len(args_name):
                    continue
                args_type.append(arg_type_tmp)
                # variable-length arguments?
                if len(args_type) != len(args_name):
                    args_name.append("")
                arg_type_tmp = ""
                continue
            elif arg_status == False and c not in ['(', ')']:
                arg_type_tmp += c
        if c == ')':
            if def_frame_size == 0:
                arg_status = True
            elif def_frame_size == 1:
                is_callback_arg = True
                arg_name_tmp += c
            def_frame_size += 1
        elif c == '(':
            def_frame_size -= 1
            if def_frame_size == 0:
                arg_type_tmp = rm_useless_space(arg_type_tmp[::-1])
                if arg_type_tmp != "" or len(args_type) != len(args_name):
                    args_type.append(arg_type_tmp)
                # variable-length arguments?
                if len(args_type) != len(args_name):
                    args_name.append("")
                fun_name_status = True  # BTW, get the func_name
            elif def_frame_size == 1:
                is_callback_arg = False
    # If it is only a name without type
    if fun_name_status == True:
        func_name = func_name[::-1]
        func_type = "void"
    # Discard meaningless characters
    if '\n' in func_type:
        func_type = func_type.split('\n')[-1]
    func_type = rm_useless_space(func_type)
    return {"func_name": func_name, "func_type": func_type, 
            "args_name": args_name[::-1], "args_type": args_type[::-1]}


def read_docfile(doc_file):
    if utils.path_exist(doc_file) != 0:
        return []
    with open(doc_file, "r", encoding="utf-8") as f:
        return f.readlines()


def clean_text(doc_text):
    '''expand abbreviations'''
    abbrs = {"don't": "do not", "Don't": "do not", "doesn't": "does not", "Doesn't": "does not", "didn't": "did not",
                "can't": "can not", "Can't": "can not", "couldn't": "could not", "Couldn't": "could not",
                "shouldn't": "should not", "Shouldn't": "should not", "should've": "should have",
                "mightn't": "might not", "mustn't": "must not", "Mustn't": "must not", "needn't": "need not",
                "haven't": "have not", "hasn't": "has not", "hadn't": "had not", 
                "you're": "you are", "You're": "you are", "you'd": "you should", "You'd": "you should",
                "it's": "it is", "It's": "it is", "isn't": "is not", "Isn't": "is not", "'ll": " will",
                "aren't": "are not", "Aren't": "are not", "won't": "will not", " n't": " not", "'d": ""}
    for abbr in abbrs:
        if re.search(abbr, doc_text):
            doc_text = re.sub(abbr, abbrs[abbr], doc_text)
    # xxx: TODO: replace bad character...
    return doc_text


def dump_json(file_path, dict_obj: dict):
    try:
        with open(file_path, "w") as f:
            f.write(json.dumps(dict_obj))
    except Exception as e:
        pass


def load_json(file_path) -> dict:
    dict_obj = dict()
    try:
        with open(file_path, "r") as f:
            dict_obj = json.load(f)
    except Exception as e:
        pass
    return dict_obj
