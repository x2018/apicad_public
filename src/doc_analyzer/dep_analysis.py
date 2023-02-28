#!/usr/bin/env python3
import hanlp, re #, pdb
from .ret_semantics import analyze_ret
from .causal_semantics import analyze_causal
from .arg_semantics import analyze_arg_pre, analyze_arg_post

'''
dependency labels: https://universaldependencies.org/u/dep/
pos labels: https://universaldependencies.org/u/pos/
'''

HanLP = hanlp.load(hanlp.pretrained.mtl.UD_ONTONOTES_TOK_POS_LEM_FEA_NER_SRL_DEP_SDP_CON_XLMR_BASE)
split_sent = hanlp.load(hanlp.pretrained.eos.UD_CTB_EOS_MUL) # end of sentence, i.e., sentence boundary detection.

class Dep_analyzer:
    def __init__(self, sentences, need_display=False):
        self.sentences = split_sent(sentences)
        self.dep_info, self.root_ids = self.__get_dep_info(need_display)

    '''
    Dispaly the output of dep_info()
    '''
    def __display_sent_dep(self, dep):
        num = 0
        for sent_dep in dep:
            num += 1
            print(f"-------------- sentence {num} --------------")
            for dep_edge in sent_dep:
                print('id: {}\tword: {}\tpos: {}\thead id: {}\thead: {}\tdeprel: {}\tchild_id: {}'.format(
                        dep_edge['id'], dep_edge['tok'], dep_edge['pos'],
                        dep_edge['head_id'], dep_edge['head_tok'], dep_edge['deprel'],
                        dep_edge['child_id']), sep='\n')
            print(f"--------------------------------------------")

    '''
    Get the formatted dependency parsing results.
    '''
    def __get_dep_info(self, need_display=False):
        # xxx: Better not. Some func names/arguments maybe sensitive to its case.
        # doc = doc.lower()
        sentences = self.sentences
        result = HanLP(sentences)
        sentence_dep = []
        root_ids = []
        for i in range(len(sentences)):
            dep_edge = []
            root_id = None
            # Unify the format.
            for n, (dep_head, deprel) in enumerate(result['dep'][i]):
                # Note：the index of hanlp is from 0, while the dep_head is from 1.
                # three items: edge.target, edge.source, edge.dep
                # format: [current_id, current_tok, current_pos, head_id, head_tok, deprel]
                dep_edge.append({
                    'id': n,
                    'tok': result['tok'][i][n],
                    'pos': result['pos'][i][n],
                    'head_id': dep_head-1,
                    'head_tok': result['tok'][i][dep_head - 1] if dep_head > 0 else "root",
                    'child_id' : [],
                    'deprel': deprel
                })
                if dep_head == 0:
                    root_id = n
            for n, dep in enumerate(dep_edge):
                if dep['head_id'] >= 0:
                    dep_edge[dep['head_id']]['child_id'].append(n)
            sentence_dep.append(dep_edge)
            root_ids.append(root_id)
        if need_display:
            print("sentence's number:", len(sentences))
            self.__display_sent_dep(sentence_dep)
        return sentence_dep, root_ids

    def __find_conjunct(self, sentence_dep, src_id):
        conjuncts = []
        src_dep = sentence_dep[src_id]
        for child_id in src_dep['child_id']:
            if sentence_dep[child_id]['deprel'] in ['conj', 'parataxis']:
                conjuncts.append(child_id)
            elif sentence_dep[child_id]['deprel'] == 'appos':
                conjuncts += self.__find_conjunct(sentence_dep, child_id)
        return conjuncts

    def retrive_related_tok(self, sentense_dep, src_id, target_relation=""):
        result_tok, result_id = "", -1
        src_edge = sentense_dep[src_id]
        for child_id in src_edge['child_id']:
            if sentense_dep[child_id]['deprel'] != target_relation:
                continue
            result_tok, result_id = sentense_dep[child_id]['tok'], child_id
            break
        return result_tok, result_id

    '''
    Preprocess sentences by their formatted dependency information.
    Output: {
        "dep_info": dep_info,
        "root": [],
        "subject": [],
        "action": [],
        "object": [],
        "clause": {head_id: [clause_id]},
    }
    '''
    def preprocess_dep(self):
        result_list = []
        for sentence_index, sentence_dep in enumerate(self.dep_info):
            result_dict = {
                "dep_info": sentence_dep,
                "root": [],
                "subject": [],
                "action": [],
                "object": [],
                "clause": {},
            }
            # First retriving root node.
            root_id = self.root_ids[sentence_index]
            if root_id != None:
                dep_edge = sentence_dep[root_id]
                result_dict['root'].append(root_id)
                result_dict['root'] += self.__find_conjunct(sentence_dep, root_id)
                if dep_edge['pos'] == "VERB":
                    result_dict['action'].append(root_id)
                    result_dict['action'] += self.__find_conjunct(sentence_dep, root_id)
            # Then analyzing all content.
            for n, dep_edge in enumerate(sentence_dep):
                deprel = dep_edge['deprel']
                # For main clause's subject & object
                if dep_edge['head_id'] in result_dict['root']:
                    if 'nsubj' in deprel:
                        result_dict['subject'].append(n)
                        result_dict['subject'] += self.__find_conjunct(sentence_dep, n)
                    elif deprel in ['obj', 'iobj']:
                        result_dict['object'].append(n)
                        result_dict['object'] += self.__find_conjunct(sentence_dep, n)
                # For sub clause   
                if deprel in ['acl', 'acl:relcl', 'advcl']:
                    if dep_edge['head_id'] not in result_dict['clause']:
                        result_dict['clause'][dep_edge['head_id']] = [n]
                    else:
                        result_dict['clause'][dep_edge['head_id']].append(n)
                    for cl_id in self.__find_conjunct(sentence_dep, n):
                        result_dict['clause'][dep_edge['head_id']].append(cl_id)   
            result_list.append(result_dict)
        return result_list


'''
Desc: Analyze sentences and extract semantics.
Input: sentences - different classified sentences in the documentation about an API.
'''
def main(sentences, cur_func, func_list={}, display=False, target_types=[]):
    feature = {}
    # For return
    if "return" in target_types and "ret" in sentences:
        # ret_feature format: {'value': [], 'cond': []}
        ret_desc = sentences['ret']
        if ret_desc != {}:
            ret_dep = Dep_analyzer(ret_desc, display)
            ret_feature = analyze_ret(ret_dep, display)
            if ret_feature != {}:
                feature['ret'] = ret_feature
    # For arguments
    if "args" in target_types and "args" in sentences:
        args_desc = {'pre': [], 'post': []}
        # Filter out irrelevant sentences
        for arg_desc in sentences['args']:
            pre_desc = ""
            post_desc = ""
            pattern_pre = re.compile(r'\b(should|must)\b (not)? *be', re.IGNORECASE)
            status_words = re.compile(r'(success|fail|error|status)', re.IGNORECASE)
            for sentence in arg_desc.split("."):
                if pattern_pre.search(sentence) != None:
                    pre_desc += sentence + ". "
                # Ignore the sentences which do not have status words.
                elif status_words.search(sentence) != None:
                    post_desc += sentence + ". "
            args_desc['pre'].append(pre_desc)
            args_desc['post'].append(post_desc)
        # arg_feature format: {'pre.check': bool, 'post.check': bool}
        arg_feature = {'arg.pre': [False for _ in range(len(cur_func['args_name']))],
                       'arg.post': [False for _ in range(len(cur_func['args_name']))]}
        for i, desc in enumerate(args_desc['pre']):
            dep = Dep_analyzer(desc, display)
            arg_feature['arg.pre'][i] = analyze_arg_pre(dep, cur_func['args_name'][i])
        # Only care the functions which do not use return as status.
        for i, desc in enumerate(args_desc['post']):
            # In case of imprecise definition analysis
            if i == len(cur_func['args_type']):
                break
            dep = Dep_analyzer(desc, display)
            arg_feature['arg.post'][i] = analyze_arg_post(dep, cur_func['args_name'][i], cur_func['args_type'][i])
        if display:
            print('arg:', arg_feature)
        for need_check in arg_feature['arg.pre']:
            if need_check == True:
                feature['arg.pre'] = arg_feature['arg.pre']
                break
        for need_check in arg_feature['arg.post']:
            if need_check == True:
                feature['arg.post'] = arg_feature['arg.post']
                break
    # For causality
    if "causality" in target_types and "causality" in sentences:
        causal_desc = ""
        # Filter out irrelevant sentences
        sensitive_word = re.compile(r' \b(must|free|clear|clean|initiate|allocate|release|open|close|' +
                                    r'frees|clears|cleans|initiates|allocates|releases|opens|closes|' +
                                    r'freed|cleared|cleaned|initiated|allocated|released|opened|closed)\b ', re.IGNORECASE)
        # causal_pattern = re.compile(r'(earlier|previous|after|before|later)', re.IGNORECASE)
        for sentence in sentences['causality'].split("."):
            if sensitive_word.search(sentence) != None: # or causal_pattern.search(sentence) != None:
                causal_desc += sentence + ". "
        # causal_feature format: {'pre': [], 'post': []}
        if causal_desc != {}:
            causal_dep = Dep_analyzer(causal_desc, display)
            causal_feature = analyze_causal(causal_dep, cur_func, func_list, display)
            if causal_feature != {}:
                feature['casuality'] = causal_feature
    return feature

if __name__ == '__main__':
    pass
