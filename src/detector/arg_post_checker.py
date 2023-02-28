#!/usr/bin/env python3

'''
"arg.post": {
    "arg_num": 1,
    "feature": [
        {
            "derefed_read": false,
            "derefed_write": false,
            // "indir_returned": false,
            "returned": false,
            "used_in_check": false,
        }
    ]
},
'''

def only_code_check(specification, complete_feature, doc_feature={}):
    feature = complete_feature['arg.post']
    alarm_text = ""
    args_need_to_check = specification['args_need_to_check']
    arg_num = feature['arg_num']
    # Check the feature of per-argument
    for arg_index in range(arg_num):
        if not arg_is_constant_or_global(complete_feature, arg_index):
            alarm_text += check_arg_feature(feature['feature'][arg_index], args_need_to_check, arg_index, doc_feature)
        else:
            pass
    return True, alarm_text


def check_arg_feature(feature, args_need_to_check, arg_index, doc_feature={}):
    alarm_text = ""
    if feature['returned'] == True: # feature['indir_returned'] == True or 
        return alarm_text
    arg_checked = feature['used_in_check']
    doc_need_to_check = False if doc_feature == {} else doc_feature[arg_index]
    if (args_need_to_check[arg_index][0] or doc_need_to_check) and not arg_checked:
        if feature['derefed_read'] == True or feature['derefed_write'] == True:
            alarm_text = f"Dereferenced without check of arg.{arg_index}.post"
        else:
            alarm_text = f"Potential lack check for arg.{arg_index}.post"
    return alarm_text


def arg_is_constant_or_global(complete_feature, num):
    if 'arg.pre' in complete_feature:
        if complete_feature['arg.pre']['feature'][num]['is_constant'] \
            or complete_feature['arg.pre']['feature'][num]['is_global']:
            return True
    return False
