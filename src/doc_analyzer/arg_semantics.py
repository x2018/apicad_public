import re

def recover_all(sentense_dep, src_id):
    src_edge = sentense_dep[src_id]
    phrase = src_edge['tok']
    pre_phrase = ""
    post_phrase = ""
    # Link all the words in any relations.
    for child_id in src_edge['child_id']:
        new_tok = recover_all(sentense_dep, child_id)
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


def recover_nsubj_phase(sentense_dep, src_id, passive=False):
    phase = ""
    target_deprel = "nsubj:pass" if passive else "nsubj"
    for child_id in sentense_dep[src_id]['child_id']:
        if sentense_dep[child_id]['deprel'] == target_deprel:
            phase = recover_all(sentense_dep, child_id)
    return phase


def arg_is_found(arg_name, phase):
    arg_is_found = True
    if arg_name in phase:
        idx = phase.index(arg_name)
        end_idx = idx + len(arg_name)
        if idx != 0 and phase[idx-1] not in ["*", " "]:
            arg_is_found = False
        if end_idx != len(phase) and phase[end_idx] not in ["*", ".", " ", "\n"]:
            arg_is_found = False
    else:
        arg_is_found = False
    return arg_is_found


def is_not_emphasis(sentense_dep, src_id):
    target_deprel = "aux"
    emphasis_words = re.compile(r'(must|should|need|require)', re.IGNORECASE)
    for child_id in sentense_dep[src_id]['child_id']:
        if sentense_dep[child_id]['deprel'] == target_deprel:
            if emphasis_words.search(sentense_dep[child_id]['tok']) != None:
                return False
    return True


def analyze_arg_pre(dep, arg_name):
    arg_need_check = False
    # Get dependency information.
    analyzed_dep = dep.preprocess_dep()
    post_causal_words = re.compile(r'(after|until|subsequent|later|then)')
    # Avoid to analyze normal functionality descriptions.
    ignore_verbs = re.compile(r'(free|release|close|use|return)')
    for i, sentence in enumerate(analyzed_dep):
        dep_info = sentence['dep_info']
        for root_id in sentence['root']:
            if is_not_emphasis(dep_info, root_id):
                continue
            # Ignore the verbs: free\release\return...
            if post_causal_words.search(dep.sentences[i]) != None \
                    or ignore_verbs.search(dep_info[root_id]['tok']) != None:
                continue
            # NOUN, NUM -> nsubj
            if dep_info[root_id]['pos'] in ['NOUN', 'NUM']:
                phase = recover_nsubj_phase(dep_info, root_id)
                arg_need_check = arg_is_found(arg_name, phase)
            # VERB -> nsubj:pass
            elif dep_info[root_id]['pos'] == "VERB":
                phase = recover_nsubj_phase(dep_info, root_id, True)
                arg_need_check = arg_is_found(arg_name, phase)
            else:
                continue
        if arg_need_check == True:
            break
    return arg_need_check


def retrive_case_word(sentense_dep, src_id):
    src_edge = sentense_dep[src_id]
    for child_id in src_edge['child_id']:
        if sentense_dep[child_id]['deprel'] != 'case':
            continue
        return sentense_dep[child_id]['tok']
    return ""


def identify_arg_id(arg_name, sentense_dep, src_id):
    src_edge = sentense_dep[src_id]
    word = src_edge['tok']

    found_arg = arg_is_found(arg_name, word)
    if found_arg:
        # ignore the preposition words that do not have the meaning can be changed.
        ignore_preposition_words = re.compile(r'\b(with|for|from|at|under|of|on)\b')
        case_word = retrive_case_word(sentense_dep, src_id)
        if ignore_preposition_words.search(case_word) == None:
            return src_id

    candidate_rels = ['appos', 'conj', 'parataxis']     
    for child_id in src_edge['child_id']:
        if sentense_dep[child_id]['deprel'] not in candidate_rels:
            continue
        new_id = identify_arg_id(arg_name, sentense_dep, child_id)
        if new_id != -1:
            return new_id
    return -1


def analyze_arg_post(dep, arg_name, arg_type):
    # Directly ignore the argument which cannot be changed by the function.
    if '*' not in arg_type:
        return False

    arg_need_check = False
    # Get dependency information.
    analyzed_dep = dep.preprocess_dep()

    # return, write, store, ...
    sensitive_verbs = re.compile(r'(store|return|write)')

    # Only concern the argument which can carry return status but not whether it can be changed.
    for sentence in analyzed_dep:
        dep_info = sentence['dep_info']
        if sentence['action'] == []:
            continue
        for root_id in sentence['root']:
            if sensitive_verbs.search(dep_info[root_id]['tok']) == None:
                continue
            arg_need_check = False
            for child_id in dep_info[root_id]['child_id']:
                if dep_info[child_id]['deprel'] not in ['obj', 'obl', 'nsubj:pass', 'nsubj']:
                    continue
                arg_id = identify_arg_id(arg_name, dep_info, child_id)
                if arg_id != -1:
                    arg_need_check = True
            if arg_need_check == True:
                break
        if arg_need_check == True:
            break
    return arg_need_check
