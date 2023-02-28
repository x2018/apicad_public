#!/usr/bin/env python3
from ..lib import keyword_utils
from ..feature_analyzer.casual_analyzer import get_chkval_cond

'''
"causality": {
    "pre.call": {
        "func_name": {
            used_as_arg: false,
            share_argument: true,
        }, ...
    }
    "post.call": {
        "func_name": {
            used_as_arg(use_target_as_arg): true,
            share_argument: false,
        }, ...
    }
}
'''


def check_causality(target, causal_feature_map, complete_feature, doc_feature={}):
    feature = complete_feature['causality']
    alarm_text = ""
    pre_functions = causal_feature_map['pre_functions']
    post_functions = causal_feature_map['post_functions']
    doc_pre_functions = []
    doc_post_functions = []
    if "causality" in doc_feature:
        doc_pre_functions = doc_feature['causality']['pre']
        doc_post_functions = doc_feature['causality']['post']

    # Skip the pre function if the function is known as usual pre functions.
    if keyword_utils.is_subsequent(target):
        alarm_text += check_causal_feature(target, feature, "pre.call", pre_functions, doc_pre_functions)
    if not is_error_handling(complete_feature, doc_feature):
        # Skip the post function if the function is known as usual post functions.
        # And lazily ignore it if the return is not used at all...
        if not keyword_utils.is_post(target) and ret_is_used(complete_feature):
            chkval_cond = get_chkval_cond(complete_feature)
            alarm_text += check_causal_feature(target,
                                               feature,
                                               "post.call",
                                               post_functions,
                                               doc_post_functions,
                                               chkval_cond=chkval_cond)
        # Record double-free-related cases.
        elif len(feature['post.call']) == 1 and keyword_utils.is_post(target) \
              and target in feature['post.call'] and target not in feature['pre.call']:
            alarm_text += f"Potential: duplicated call of {target} in post.call. "
    return True, alarm_text


def is_global(complete_feature=None):
    if complete_feature != None:
        if 'retval' in complete_feature and 'ctx' in complete_feature['retval']:
            if complete_feature['retval']['ctx']['indir_returned'] \
                    or complete_feature['retval']['ctx']['returned'] \
                    or complete_feature['retval']['ctx']['stored_not_local']:
                return True
    return False


def ret_is_used(complete_feature=None):
    if complete_feature != None and not is_global(complete_feature):
        if 'retval' in complete_feature and 'ctx' in complete_feature['retval']:
            if complete_feature['retval']['check']['checked'] or \
                    complete_feature['retval']['check']['indir_checked'] or \
                    complete_feature['retval']['ctx']['derefed_read'] or \
                    complete_feature['retval']['ctx']['derefed_write'] or \
                    complete_feature['retval']['ctx']['used_in_bin'] or \
                    complete_feature['retval']['ctx']['used_in_call']:
                return True
        else:  # no return..
            return True
    return False


def check_causal_feature(target, feature, causal_type, functions, doc_functions, chkval_cond=None):
    if ignore_causal(target, causal_type, feature[causal_type]):
        return ""
    alarm_text = ""
    for causal_func in functions:
        # If the frequency is 1
        frequency = functions[causal_func] if causal_type == "pre.call" \
                        else functions[causal_func][0]
        if frequency == 1:
            continue
        # For post.call # chkval_cond != "no_check" and
        if chkval_cond != None and functions[causal_func][1] != {}:
            if chkval_cond not in functions[causal_func][1]:
                continue
        # Consider the direct variants
        for func in feature[causal_type]:
            if causal_func in func:
                causal_func = func
                break
        # Check whether have the causal function
        if causal_func not in feature[causal_type]:
            alarm_text += f"Lack {causal_type}: {causal_func}. "
        # If there are many classes, then we only detect the most-frequent
        if len(functions) > 2:
            break
    if alarm_text == "" and len(doc_functions) > 0:
        has_doc_func = False
        for doc_func in doc_functions:
            if doc_func in feature[causal_type]:
                has_doc_func = True
        if not has_doc_func:
            alarm_text += f"Lack one of them in {causal_type}: {doc_functions}. (by documents spec.) "
    return alarm_text


def ignore_causal(target, causal_type, functions):
    for func in functions:
        if causal_related(func, causal_type) \
                and has_same_prefix(target, func, causal_type):
            return True
    return False


def causal_related(func_name, causal_type):
    if causal_type == "pre.call":
        if keyword_utils.is_pre(func_name):
            return True
    else:
        if keyword_utils.is_post(func_name):
            return True
    return False


def has_same_prefix(target, causal_func, causal_type):
    length = len(target) if len(target) > len(causal_func) else len(target)
    idx = 0
    if causal_type == "post.call":
        if '_' in target:
            idx = len(target) - target[::-1].index('_') - 1
        else:
            for idx in range(length):
                if not keyword_utils.is_pre_seq(target[idx:]):
                    break
        if idx < length:
            if keyword_utils.is_pre_seq(target[idx:]) == keyword_utils.is_post(causal_func[idx:]) \
                    or (idx > 0 and target[:idx] == causal_func[:idx]):
                return True
    else:
        if '_' in target:
            idx = len(target) - target[::-1].index('_') - 1
        else:
            for idx in range(length):
                if not keyword_utils.is_subsequent(target[idx:]):
                    break
        if idx < length:
            if keyword_utils.is_subsequent(target[idx:]) == keyword_utils.is_pre(causal_func[idx:]) \
                    or (idx > 0 and target[:idx] == causal_func[:idx]):
                return True
    return False


def is_error_handling(complete_feature, doc_feature):
    if "ret" not in doc_feature:
        return False
    if "success" not in doc_feature['ret']['cond'] \
            and "fail" not in doc_feature['ret']['cond']:
        return False
    if 'retval' in complete_feature:
        ret_check = complete_feature['retval']['check']
        if ret_check['checked'] and not ret_check['compared_with_non_const'] \
                and not ret_check['indir_checked']:
            retval = ret_check['compared_with_const']
            retcond = ret_check['check_cond']
            if retval in doc_feature['ret']['value']:
                idx = doc_feature['ret']['value'].index(retval)
                cond = doc_feature['ret']['cond'][idx]
                if (cond == "success" and retcond in ["ne", "lt", "gt"]) \
                        or (cond == "fail" and retcond in ["eq", "le", "ge"]):
                    return True
    return False
