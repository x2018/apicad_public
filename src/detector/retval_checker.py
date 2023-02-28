#!/usr/bin/env python3

'''
"retval": {
    "check" : {...},
    "ctx": {...},    
}
e.g.
    "check": {
        "check_cond": eq|ne|gt|ge|lt|le,
        "checked": true,
        "indir_checked": bool, 
        "compared_with_const": 0,
        "compared_with_non_const": false,
    }
    "ctx": {
        "derefed_read": false,
        "derefed_write": false,
        "indir_returned": false,
        "returned": false,
        // "stored": true,
        "stored_not_local": false,
        "used_in_bin": false,
        "used_in_call": false,
    }
'''


def check_retval(func_name, complete_feature, ret_specification, doc_feature={}):
    need_check = False
    if ret_specification['no_need_to_check_if_same_in_post']:
        if func_name in complete_feature['causality']['post.call']:
            return False, "do not need to be checked when there is a same call in post..."
        elif ret_specification['no_same_in_post_need_to_check']:
            need_check = True
    feature = complete_feature['retval']
    ret_checked = feature['check']['checked'] or feature['check']['indir_checked']
    if need_check or ret_specification['need_to_check'][0] or ret_need_check(doc_feature):
        if not ret_checked:
            # Only tolerate the cases which are directly returned.
            if feature['ctx']['returned'] == True: # feature['ctx']['indir_returned'] == True or
                return False, "returned..."
            if (feature['ctx']['derefed_read'] or feature['ctx']['derefed_write']):
                alarm_text = "Dereferenced read/write the return value without check. "
            else:
                alarm_text = "Lacking proper check for the return value. "
            return True, alarm_text
        else:
            valid_chkvals = ret_specification['valid_chkvals']
            if valid_chkvals == {}:
                return False, ""
            if feature['check']['checked']:
                if feature['check']['compared_with_non_const']:
                    chkval = "non_const"
                # Consider the equivalent check value
                elif feature['check']['check_cond'] in ["gt", "le"]:
                        chkval = feature['check']['compared_with_const'] + 1/4
                elif feature['check']['check_cond'] in ["ge", "lt"]:
                    chkval = feature['check']['compared_with_const'] - 1/4
                else:
                    chkval = feature['check']['compared_with_const']
            elif feature['check']['indir_checked']:
                chkval = "indir_chk"
            doc_retvals = ret_values(doc_feature)
            if chkval not in valid_chkvals and chkval not in doc_retvals:
                # xxx: supress some false positives brought by frequent-based assumption.
                return True, f"The check condition for the return value is potential wrong. "
    return False, ""


def ret_need_check(doc_feature):
    if "value" not in doc_feature:
        return False
    elif doc_feature['value'] != []:
        return True
    # unintentional cases happend...
    return False


def ret_values(doc_feature):
    if "value" not in doc_feature:
        return []
    return doc_feature['value']
