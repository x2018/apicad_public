import re

'''
Convert common string to int.
If we do not know the type, then we just return "" and use the type of
the function to predict the potential value. 
'''
def convert_str2value(string: str):
    low_string = string.lower().split(" ")
    # xxx: TODO: non-/negative- ..., one/two/three ?
    zero_assumed_words = re.compile(r'\b(null|false|zero)\b') # r'\b(non)?-?negative|positive\b'
    for word in low_string:
        if zero_assumed_words.search(word) != None: # "pointer"
            return 0
        elif word == "true":
            return 1
        elif word == "eof":
            return -1
    return ""


def extract_num(string: str):
    ret = string
    try:
        for word in string.split(" "):
            cleaned_word = re.findall("^[-|+]?\d+$", word)
            if cleaned_word != []:
                return int(cleaned_word[0])
    except ValueError:
        pass
    return ret


def need_flip_cond(dep_info, action_ids):
    for action_id in action_ids:
        for child_id in dep_info[action_id]['child_id']:
            child_dep = dep_info[child_id]
            if child_dep['deprel'] == "advmod" \
                    and child_dep['pos'] == "PART" \
                    and child_dep['tok'].lower() == "not":
                return True
    return False


'''
Unify the check condition: "success" or "fail"...
'''
def unify_condition(dep_info, target_id, flip=False):
    cond, cond_id = [], []
    for child_id in dep_info[target_id]['child_id']:
        # Only focus on current modifiers but not other objects/actions.
        if dep_info[child_id]['deprel'] in ['conj', 'parataxis', 'obj']:
            continue
        cur_cond = ""
        new_flip = flip
        if need_flip_cond(dep_info, [child_id]):
            new_flip = not flip
        token = dep_info[child_id]['tok']
        if len(token) > 6 and (token[:7] in ["success", "correct"]):
            cur_cond = "success" if new_flip == False else "fail"
        elif (len(token) > 4 and (token[:3] == "err" or token[:4] == "fail")) \
            or (token == "invalid"):
            cur_cond = "fail" if new_flip == False else "success"
        if cur_cond != "":
            cond.append(cur_cond)
            cond_id.append(child_id)
        else:
            cur_cond, cur_cond_id = unify_condition(dep_info, child_id, new_flip)
            cond += cur_cond
            cond_id += cur_cond_id
    return cond, cond_id


def recover_condition(dep_info, target_ids, cond_flip=False):
    conds = {'cond': [], 'cond_id': []}
    for target_id in target_ids:
        ret_cond, cond_id = unify_condition(dep_info, target_id, cond_flip)
        if ret_cond != "":
            conds['cond'] += ret_cond
            conds['cond_id'] += cond_id
    return conds


'''
Scan the sentence and make the NUM as potential return value.
'''
def scan_rough_num(ret_dict, dep_info, cond_flip):
    for edge in dep_info:
        if edge['pos'] == 'NUM':
            value = extract_num(edge['tok'])
            if type(value) != int:
                value = convert_str2value(value)
            if value != "" and value not in ret_dict['value']:
                ret_dict['value'].append(value)
                ret_dict['value_id'].append(edge['id'])
                cond = recover_condition(dep_info, [edge['id']], cond_flip)
                if len(cond['cond']) > 0:
                    ret_dict['cond'].append(cond['cond'][-1])
                    ret_dict['cond_id'].append(cond['cond_id'][-1])
                else:
                    ret_dict['cond'].append("roughly")
                    ret_dict['cond_id'].append(-1)
        elif edge['tok'] == "NULL" and 0 not in ret_dict['value']:
            value = 0
            cond = recover_condition(dep_info, [edge['id']], cond_flip)
            ret_dict['value'].append(value)
            ret_dict['value_id'].append(edge['id'])
            if len(cond['cond']) > 0:
                ret_dict['cond'].append(cond['cond'][-1])
                ret_dict['cond_id'].append(cond['cond_id'][-1])
            else:
                ret_dict['cond'].append("roughly")
                ret_dict['cond_id'].append(-1)
    return ret_dict


def recover_phrase(sentense_dep, src_id, consider_conj=False):
    src_edge = sentense_dep[src_id]
    phrase = src_edge['tok']
    pre_phrase = ""
    post_phrase = ""
    candidate_rels = ['det', 'amod', 'nmod', 'nummod', 'advmod', 'appos', # 'punct', # 'cc', 
                    'fixed', 'flat', 'compound', 'case', 'obl']
    if consider_conj:
        candidate_rels += ['conj', 'parataxis']
    # Link all the words in candidate relations.
    for child_id in src_edge['child_id']:
        if sentense_dep[child_id]['deprel'] in candidate_rels:
            new_tok = recover_phrase(sentense_dep, child_id)
            if child_id > src_id:
                post_phrase = post_phrase if post_phrase == "" else post_phrase + " "
                post_phrase += new_tok
            else:
                pre_phrase = pre_phrase if pre_phrase == "" else pre_phrase + " "
                pre_phrase += new_tok
    if post_phrase != "":
        phrase += " " + post_phrase
    if pre_phrase != "":
        phrase = pre_phrase + " " + phrase
    return phrase


'''
Insert new item to the condition dict according to cond_id.
'''
def insert2cond(sent_cond, new_cond_id, new_cond):
    for j, cond_id in enumerate(sent_cond['cond_id']):
        if cond_id < new_cond_id:
            continue
        sent_cond['cond'].insert(j, new_cond)
        sent_cond['cond_id'].insert(j, new_cond_id)
        break
    if len(sent_cond['cond_id']) == 0:
        sent_cond['cond'].append(new_cond)
        sent_cond['cond_id'].append(new_cond_id)
    return sent_cond


def get_ret_dict(dep_info, target_ids, cond_flip=False):
    values, value_ids, conds, cond_ids = [], [], [], []
    # Recover [ret_value, ret_condition].
    # => {"value": [...], "cond": [...]}
    for target_id in target_ids:
        # Skip other actions except for "return".
        if dep_info[target_id]['head_tok'] != 'root' and \
                "return" not in dep_info[target_id]['head_tok'].lower():
            continue
        phrase = recover_phrase(dep_info, target_id)
        value = ""
        # Attempts to extract num from the string.
        value = extract_num(phrase)
        if type(value) != int:
            value = convert_str2value(phrase)
        if value != "":
            if value not in values:
                values.append(value)
                value_ids.append(target_id)
                cond = recover_condition(dep_info, [target_id], cond_flip)
                if len(cond['cond']) == 1:
                    conds.append(cond['cond'][0])
                    cond_ids.append(cond['cond_id'][0])
                else:
                    conds.append("roughly")
                    cond_ids.append(-1)
        else:
            # Record it but finally will be cleaned.
            values.append(phrase)
            value_ids.append(target_id)
            cond = recover_condition(dep_info, [target_id], cond_flip)
            if len(cond['cond']) == 1:
                conds.append(cond['cond'][0])
                cond_ids.append(cond['cond_id'][0])
            else:
                conds.append("roughly")
                cond_ids.append(-1)
    return {
        'value': values,
        'value_id': value_ids,
        'cond': conds,
        'cond_id': cond_ids,
    }


def merge_feature_list(feature_list: list):
    result = {'value':[], 'cond': []}
    # Merge all sub dict.
    for per_dict in feature_list:
        for i, value in enumerate(per_dict['value']):
            if value not in result['value']:
                result['value'].append(value)
                result['cond'].append(per_dict['cond'][i])
            else:
                index = result['value'].index(value)
                if result['cond'][index] == "":
                    result['cond'][index] = per_dict['cond'][i]

    # Roughly update remaining conditions.
    if "success" not in result['cond'] and "fail" in result['cond']:
        for i, cond in enumerate(result['cond']):
            if cond in ["", "roughly"]:
                result['cond'][i] = "success"
    elif "success" in result['cond'] and "fail" not in result['cond']:
        for i, cond in enumerate(result['cond']):
            if cond in ["", "roughly"]:
                result['cond'][i] = "fail"

    # Finally, clean non-int value and its condition.
    ori_len = len(result['value'])
    cleaned_num = 0
    for i in range(ori_len):
        index = i - cleaned_num
        if type(result['value'][index]) != int:
            result['value'].pop(index)
            result['cond'].pop(index)
            cleaned_num += 1
    return result
                

'''
Analyze the semantic inside the description about return value.
'''
def analyze_ret(dep, display=False):
    # Get dependency information.
    analyzed_dep = dep.preprocess_dep()
    # Init ret feature list.
    feature_list = []
    for sentence in analyzed_dep:
        # Init ret feature dict.
        ret_dict = {'value': [], 'value_id': [], 'cond': [], 'cond_id': []}
        dep_info = sentence['dep_info']

        # Check whether this sentence is return-related.
        if sentence['action'] != []:
            has_return = False
            for verb_id in sentence['action']:
                if "return" in dep_info[verb_id]['tok'].lower():
                    has_return = True
                    break
            if not has_return:
                continue
        # Check the verb whether exists (return?).
        cond_flip = False  # Should we flip the condition?
        sent_cond = {'cond': [], 'cond_id': []}

        # Normal handling.
        if sentence['action'] != []:
            cond_flip = need_flip_cond(dep_info, sentence['action'])
            sent_cond = recover_condition(dep_info, sentence['action'], cond_flip)
            ret_dict = get_ret_dict(dep_info, sentence['object'], cond_flip)
        else:
            ret_dict = get_ret_dict(dep_info, sentence['root'])
            cond_flip = need_flip_cond(dep_info, sentence['root'])
            sent_cond['cond'], sent_cond['cond_id'] = ret_dict['cond'].copy(), ret_dict['cond_id'].copy()

        # Heuristics to deal with inaccurate Dep etc.
        # 1. Attempts scan the roughly num as the value.
        ret_dict = scan_rough_num(ret_dict, dep_info, cond_flip)
        # 2. Adjust current conditions.
        for i, cond_id in enumerate(ret_dict['cond_id']):
            if cond_id == -1:
                continue
            logic_order = True if cond_id > ret_dict['value_id'][i] else False
            for j, value_id in enumerate(ret_dict['value_id']):
                if j == i:
                    continue
                if logic_order:
                    if cond_id > value_id and value_id > ret_dict['value_id'][i]:
                        sent_cond = insert2cond(sent_cond, ret_dict['cond_id'][i], ret_dict['cond'][i])
                        ret_dict['cond'][i] = "roughly"
                elif cond_id < value_id and value_id < ret_dict['value_id'][i]:
                    sent_cond = insert2cond(sent_cond, ret_dict['cond_id'][i], ret_dict['cond'][i])
                    ret_dict['cond'][i] = "roughly"
        # Update the conditions based on the clause of action.
        if sent_cond['cond'] != []:
            if len(ret_dict['cond']) < len(sent_cond['cond']):
                i = len(ret_dict['cond']) - 1
            else:
                i = len(sent_cond['cond']) - 1
            for j in range(len(ret_dict['cond'])):
                if i < 0:
                    break
                cur_index = len(ret_dict['cond']) - j - 1
                cc_tok, cc_id = dep.retrive_related_tok(dep_info, ret_dict['value_id'][cur_index], "cc")
                not_conj = dep_info[ret_dict['value_id'][cur_index]]['deprel'] != "conj"
                if ret_dict['cond'][cur_index] == "roughly" \
                        and (sent_cond['cond_id'][i] > ret_dict['value_id'][cur_index] \
                            or not_conj):
                    ret_dict['cond'][cur_index] = sent_cond['cond'][i]
                    ret_dict['cond_id'][cur_index] = sent_cond['cond_id'][i]
                # If there is not a conjunct or the condition id is the same with the original id,
                # change to the next condition.
                if (cc_tok in ["and", "or"] and cc_id < sent_cond['cond_id'][i]) or not_conj \
                        or ret_dict['cond_id'][cur_index] == sent_cond['cond_id'][i]:
                    i -= 1

        # Clear value id & update feature list.
        ret_dict.pop('value_id')
        ret_dict.pop('cond_id')
        feature_list.append(ret_dict)

    # Merge all feature of sentences.
    feature_dict = merge_feature_list(feature_list)
    if display:
        print('return:', feature_dict)
    if feature_dict['value'] == []:
        return {}
    return feature_dict
