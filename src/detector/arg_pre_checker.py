#!/usr/bin/env python3
from ..lib import keyword_utils

'''
"arg.pre": {
    "arg_num": n,
    "feature": [...],
    // "has_relation": true,
    // "relations": [[0, 1], ...]
},
"feature" - e.g.
    "feature": [
        {
            "check": {
                // "check_cond": "",
                "checked": false,
                // "compared_with_const": 0,
                // "compared_with_non_const": false
            },
            "is_alloca": false,
            "is_global": false,
            "is_constant": false,
            // "arg_value": -1,
        }
    ],
'''


def only_code_check(func_name, specification, complete_feature, doc_feature={}):
    if specification == {}:
        return False, ""
    feature = complete_feature['arg.pre']
    alarm_text = ""
    arg_num = feature['arg_num']
    args_need_to_check = specification['args_need_to_check']
    if arg_num != len(args_need_to_check):
        # internal error?
        return False, ""

    # Check the feature of per-argument
    if not keyword_utils.is_post(func_name):
        for num in range(arg_num):
            alarm_text += check_arg_feature(feature['feature'][num], args_need_to_check[num][0], num, doc_feature)
    else:
        for num in range(arg_num):
            if complete_feature['arg.pre']['feature'][num]['is_alloca']:
                alarm_text += f"Potential: arg {num} is on stack and dealloced. "
    return True, alarm_text


def check_arg_feature(feature, args_need_to_check, arg_index, doc_feature={}):
    checked = feature['check']['checked']
    doc_need_to_check = False if doc_feature == {} else doc_feature[arg_index]
    if args_need_to_check or doc_need_to_check:
        if not checked and not feature['is_global']: # and not feature['is_constant']:
            return f"violate the most-frequent check for arg.{arg_index}.pre. "
    return ""
