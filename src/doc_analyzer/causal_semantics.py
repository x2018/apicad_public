import re
from ..lib import keyword_utils

def recover_full_name(ori_sentence, func):
    if func not in ori_sentence:
        return ""
    index = ori_sentence.index(func)
    end_index = index + len(func) - 1
    while index >= 0 and ori_sentence[index] != " ":
        index -= 1
    return ori_sentence[index+1:end_index]


def recover_causal(pre_phase, post_phase):
    pre_causal_words = re.compile(r'(previous|before|earlier)', re.IGNORECASE)
    post_causal_words = re.compile(r'(after|subsequent|later|until|then)', re.IGNORECASE)
    if post_causal_words.search(post_phase) != None:
        return "reverse"
    if pre_causal_words.search(pre_phase) != None:
        return "reverse"
    # if pre_causal_words.search(post_phase) != None:
    #     return "normal"
    # if post_causal_words.search(pre_phase) != None:
    #     return "normal"
    return "normal"


def recover_relations(sentense_dep, src_id):
    src_edge = sentense_dep[src_id]
    phrase = src_edge['tok']
    pre_phrase = ""
    post_phrase = ""
    candidate_rels = ['advmod', 'appos', 'cc', 'obj', 'obl', 'fixed', 'xcomp',
                      'flat', 'compound', 'case', 'conj', 'parataxis']
    # Link all the words in candidate relations.
    for child_id in src_edge['child_id']:
        if sentense_dep[child_id]['deprel'] not in candidate_rels:
            continue
        new_tok = recover_relations(sentense_dep, child_id) # sentense_dep[child_id]['tok']
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
Get the causal relationship based on the action word and logical words.
'''
def get_causal(feature_dict, cur_func, func_list, dep_info, word_id, ori_sentence=""):
    is_pre = keyword_utils.is_pre(cur_func['func_name'])
    is_post = keyword_utils.is_post(cur_func['func_name'])
    action_tok = dep_info[word_id]['tok']
    pre_phase = ""
    post_phase = ""

    # ignore to analyze the return related sentences and the actions about notice signs
    ignore_verbs = re.compile(r'(have|has|return|see|mention|note)')
    if ignore_verbs.search(action_tok) != None:
        return feature_dict
    # Acquire the phases
    for child_id in dep_info[word_id]['child_id']:
        if dep_info[child_id]['deprel'] not in ['nsubj:pass', 'nsubj', 'obj', 'obl', 'acl', 'advcl']:
            continue
        phase = recover_relations(dep_info, child_id)
        if cur_func['func_name'] in phase:
            continue
        else:
            if child_id > word_id:
                post_phase += phase
            else:
                pre_phase += phase

    # Confirm the causal order of the sentence
    causal_order = "normal"
    func_pattern = re.compile(r'[0-9,a-z,A-Z,_]+\(', re.IGNORECASE)
    pre_action_pattern = re.compile(r'(allocate|open|create|initiate)', re.IGNORECASE)
    post_action_pattern = re.compile(r'(free|release|close|clear|clean)', re.IGNORECASE)
    if "ed" == action_tok[-2:]:
        if pre_action_pattern.search(action_tok) != None:
            causal_order = "reverse"
        elif post_action_pattern.search(action_tok) != None:
            causal_order = "normal"
        else:
            causal_order = recover_causal(pre_phase, post_phase)
    else:
        if pre_action_pattern.search(action_tok) != None:
            causal_order = "normal"
        elif post_action_pattern.search(action_tok) != None:
            causal_order = "reverse"
        else:
            causal_order = recover_causal(pre_phase, post_phase)

    # Update the feature dict
    if causal_order == "normal":
        for func in func_pattern.findall(post_phase):
            func_name = recover_full_name(ori_sentence, func)
            if not is_post and func_name != cur_func['func_name'] and func_name in func_list:
                feature_dict['post'].append(func_name)
        for func in func_pattern.findall(pre_phase):
            func_name = recover_full_name(ori_sentence, func)
            if not is_pre and func_name != cur_func['func_name'] and func_name in func_list:
                feature_dict['pre'].append(func_name)
    else:
        for func in func_pattern.findall(post_phase):
            func_name = recover_full_name(ori_sentence, func)
            if not is_pre and func_name != cur_func['func_name'] and func_name in func_list:
                feature_dict['pre'].append(func_name)
        for func in func_pattern.findall(pre_phase):
            func_name = recover_full_name(ori_sentence, func)
            if not is_post and func_name != cur_func['func_name'] and func_name in func_list:
                feature_dict['post'].append(func_name)
    return feature_dict


'''
Analyze the semantic inside the description about causality.
'''
def analyze_causal(dep, cur_func, func_list={}, display=False):
    # Get dependency information.
    analyzed_dep = dep.preprocess_dep()
    # Init the feature dict.
    feature_dict = {'pre': [], 'post': []}
    for i, sentence in enumerate(analyzed_dep):
        if sentence['root'] != sentence['action']:
            continue
        dep_info = sentence['dep_info']
        # Skip the complex sentences which have two or more verbs.
        actions_num = 0
        for root_id in sentence['root']:
            if dep_info[root_id]['deprel'] in ['root', 'conj']:
                actions_num += 1
        if actions_num > 1:
            continue
        for root_id in sentence['root']:
            feature_dict = get_causal(feature_dict, cur_func, func_list, dep_info, root_id, dep.sentences[i])
    feature_dict['pre'] = list(set(feature_dict['pre']))
    feature_dict['post'] = list(set(feature_dict['post']))
    if display:
        print('causality:', feature_dict)
    if len(feature_dict['pre']) == 0 and len(feature_dict['post']) == 0:
        return {}
    return feature_dict
