#!/usr/bin/env python3
import os, subprocess
from ..lib import utils, doc_utils

'''
get per-class-of-function infomation in source of (Linux manual) of libc
    - NAME/SYNOPSIS/DESCRIPTION/RETURN VALUE/NOTES/BUGS
It is similar to the handling for openssl in some way.
TODO: unify the processing as one file?
concern: may have some absent situations.
'''

def man2text(doc_dir):
    doc_files = []
    doc_dir = os.path.join(doc_dir, "glibc/share/man/man3")
    if utils.path_exist(doc_dir) == -1:
        return []
    for dirpath, _, files in os.walk(doc_dir):
        for file in files:
            if len(file) < 4 or file[-4:] == '.txt':
                continue
            cur_path = os.path.join(dirpath, file)
            if utils.path_exist(cur_path + '.txt', True) < 0:
                with open(cur_path + '.txt', "w") as f:
                    cmd = ['man', cur_path]
                    ret = subprocess.run(cmd, stdout=f)
                    # xxx: "man: -:1: warning: failed .so request" ?
                    if ret.returncode != 0:
                        raise Exception("Failure during running `man`.")
            doc_files.append(cur_path + '.txt')
    return doc_files


def update_desc(content, desc, definitions, content_type):
    # clean text
    content = doc_utils.clean_text(content)
    for para in content.split("\n\n"):
        for key in definitions.keys():
            if key + "()" in para:
                para = para.replace('‐\n', '')
                para = para.replace('\n', ' ')
                if key not in desc:
                    desc[key] = definitions[key]
                    desc[key][content_type] = para
                elif content_type in desc[key]:
                    desc[key][content_type] += '\n' + para
                else:
                    desc[key][content_type] = para
    return desc


''' load all the content of a doc file for subsequent processing '''
def load_doc_content(doc_file):
    contents = {}
    cur_item = ""
    doc_lines = doc_utils.read_docfile(doc_file)
    for line in doc_lines:
        if len(line) > 1 and line[0] != " ":
            cur_item = line[:-1]
            contents[cur_item] = ""
            continue
        if cur_item != "":
            if line[0] == '\n':
                contents[cur_item] += '\n'
            else:
                # drop the useless indent
                contents[cur_item] += doc_utils.rm_useless_space(line, end=False)
    return contents


''' split each API from each class of documentation '''
def split_api(doc_file):
    names = []
    definitions = {}
    descriptions = {}
    contents = load_doc_content(doc_file)
    # 1. acquire API's names
    if 'NAME' not in contents:
        return [], {}
    # 2. acquire each API's definition
    if 'SYNOPSIS' not in contents:
        return [], {}
    for synopsis in contents['SYNOPSIS'].split(';\n'):
        # outdated: [type, func_name, arg1, arg2, ..., argn]
        # now: {"func_name": .., "func_type": .., "args_name": .., "args_type": ..}
        definition = doc_utils.get_definition(synopsis)
        if definition['func_name'] == '':
            continue
        if definition['func_name'] not in names:
            names.append(definition['func_name'])
        else:
            continue
        definitions[definition['func_name']] = definition
    # 3. acquire each API's description
    ''' For simplicity, now only concern: 'DESCRIPTION', 'RETURN VALUES'... '''
    if 'DESCRIPTION' in contents:
        descriptions = update_desc(contents['DESCRIPTION'], descriptions, definitions, "desc")
    if 'RETURN VALUES' in contents:
        descriptions = update_desc(contents['RETURN VALUES'], descriptions, definitions, "ret_desc")
    if 'RETURN VALUE' in contents:
        descriptions = update_desc(contents['RETURN VALUE'], descriptions, definitions, "ret_desc")
    return names, descriptions


# doc_dir - the storage directory of data
def handle_glibc(doc_dir, outdir):
    print("==================================================")
    print("====         Preprocessing glibc info        =====")
    ''' initialization '''
    preprocess_dir = os.path.join(outdir, "glibc")
    utils.mkdir(preprocess_dir)
    doc_files = man2text(doc_dir)
    ''' preprocess documentation '''
    total_apis = []
    analyzed_apis = []
    for doc_file in doc_files:
        cur_apis, descriptions = split_api(doc_file)
        total_apis += cur_apis
        for func_name in descriptions:
            if func_name not in analyzed_apis:
                analyzed_apis.append(func_name)
                func_feature_file = os.path.join(preprocess_dir, f"{func_name}.json")
                doc_utils.dump_json(func_feature_file, descriptions[func_name])

    print(f"Total number of functions: {len(set(total_apis))}")
    print(f"Success number of functions: {len(analyzed_apis)}")
    print("==================================================")

    return list(set(total_apis))
