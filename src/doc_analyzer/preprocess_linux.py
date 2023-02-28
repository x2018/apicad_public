#!/usr/bin/env python3
import os
from ..lib import utils, doc_utils

'''
desc: infomation of linux
    per-function infomation in Linux Core API page
        - basic description, Parameters, Description, Note, Context, Return
    concern: may have some absent situations
'''

def preprocess_linux_apidoc(info, preprocess_dir):
    definition = ""
    description = {}
    desc_keyword = None
    desc_status = False
    definition = info[0]
    # outdated: [type, func_name, arg1, arg2, ..., argn]
    # now: {"func_name": .., "func_type": .., "args_name": .., "args_type": ..}
    description = doc_utils.get_definition(definition)
    if description['func_name'] == "":
        return ""
    for line in info[1:]:
        if line == "Description\n" or line == "Note\n" or line == "Context\n":
            desc_keyword = "desc"
            if not desc_status:
                desc_status = True
        elif line == "Return\n":
            desc_keyword = "ret_desc"
            if not desc_status:
                desc_status = True
        # xxx: what about Parameters?
        elif desc_status:
            content = line.replace("\n", " ") if line != "\n" else line
            if desc_keyword not in description:
                description[desc_keyword] = content
            else:
                description[desc_keyword] += content
    # clean text
    for key in description:
        if key in ["desc", "ret_desc"]:
            description[key] = doc_utils.clean_text(description[key])
    func_feature_file = os.path.join(preprocess_dir, f"{description['func_name']}.json")
    doc_utils.dump_json(func_feature_file, description)
    return description['func_name']


# doc_dir - the storage directory of data
def handle_linux(doc_dir, outdir):
    print("==================================================")
    print("====         Preprocessing Linux info        =====")
    ''' initialization '''
    preprocess_dir = os.path.join(outdir, "linux")
    utils.mkdir(preprocess_dir)
    doc_file = os.path.join(doc_dir, "linux/linux_api.txt")
    doc_lines = doc_utils.read_docfile(doc_file)
    ''' preprocess documentation '''
    total_apis = []
    func_info = []
    for line in doc_lines:
        if line[:20] == "=" * 20:
            if func_info != []:
                func_name = preprocess_linux_apidoc(func_info, preprocess_dir)
                if func_name != "":
                    total_apis.append(func_name)
                func_info = []
        elif line != "\n":
            func_info.append(line)
    if func_info != []:
        func_name = preprocess_linux_apidoc(func_info, preprocess_dir)
        if func_name != "":
            total_apis.append(func_name)

    print(f"Total number of functions: {len(total_apis)}")
    print("==================================================")

    return total_apis
