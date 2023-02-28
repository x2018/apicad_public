#!/usr/bin/env python3
import os
from ..lib import doc_utils


'''
input: {"func_name": .., "func_type": .., "args_name": .., "args_type": .., "desc": ...}
output: {"ret": .., "args": .., "causality": ..}
'''
def classify_sentences(desc, func_list={}):
    args_name = desc['args_name']
    func_name = desc['func_name']
    func_type = desc['func_type']
    normal_desc = desc['desc']
    sentence_classes = {'ret': "", 'args': [], 'causality': ""}
    need_to_retrive_return = False
    for _ in range(len(args_name)):
        sentence_classes['args'].append("")

    if func_type != "void":
        if "ret_desc" in desc:
            sentence_classes['ret'] = desc['ret_desc']
        else:
            need_to_retrive_return = True
    for sentence in normal_desc.split("."):
        sentence = sentence.strip("\n")
        if need_to_retrive_return and "return" in sentence:
            sentence_classes['ret'] += sentence + ". "
        is_arg_related = [False for _ in range(len(args_name))]
        is_causal_related = False
        split_words = sentence.split(" ")
        for word in split_words:
            if not is_causal_related and '(' in word and word.split("(")[0] != func_name and word.split("(")[0] in func_list:
                sentence_classes['causality'] += sentence + ". "
                is_causal_related = True
            elif is_arg_related != [True for _ in range(len(args_name))]:
                arg_name = doc_utils.rm_useless_asterisk(word)
                arg_index = -1 if arg_name not in args_name else args_name.index(arg_name)
                if arg_index != -1:
                    sentence_classes['args'][arg_index] += sentence + ". "
                    is_arg_related[arg_index] = True
                else:
                    continue
            else:
                break
    return sentence_classes


'''
analyze and extract semantics from different classes of sentences.
'''
def analyze_sentences(cur_func, sent_classes, func_list={}, dispaly=False, target_types=[]):
    # put the model loading here to prevent unnecessary costs
    from . import dep_analysis
    return dep_analysis.main(sent_classes, cur_func, func_list, dispaly, target_types)


'''
combine the old_feature and new_feature.
'''
def update_feature(old_feature, new_feature):
    result_feature = {}
    for key in old_feature:
        if key not in new_feature:
            result_feature[key] = old_feature[key]
    for key in new_feature:
        result_feature[key] = new_feature[key]
    return  result_feature

def main(out_dir, preprocess_dir, func_list={}, display=False, semantic_type=""):
    feature_file = os.path.join(out_dir, "doc_feature.json")
    doc_feature = {}
    if os.path.exists(feature_file):
        doc_feature = doc_utils.load_json(feature_file)
    target_types = ["causality", "args", "return"]
    if semantic_type in target_types:
        target_types = [semantic_type]
    # Extracting semantics from the preprocessed files.
    for dirpath, _, files in os.walk(preprocess_dir):
        for file in files:
            if len(file) < 5 or file[-5:] != ".json":
                continue
            cur_path = os.path.join(dirpath, file)
            # {"func_name": .., "func_type": .., "args_name": .., "args_type": ..}
            cur_func = doc_utils.load_json(cur_path)
            if "desc" in cur_func:
                sent_classes = classify_sentences(cur_func, func_list)
            elif "ret_desc" in cur_func:
                sent_classes = {"ret": cur_func['ret_desc']}
            else:
                continue
            # xxx: save func_type and args_type into feature?
            tmp_feature = analyze_sentences(cur_func, sent_classes, func_list, display, target_types)
            # Update the feature.
            if cur_func['func_name'] not in doc_feature:
                new_feature = tmp_feature
            else:
                ori_feature = doc_feature[cur_func['func_name']]
                new_feature = update_feature(ori_feature, tmp_feature)
            if new_feature != {}:
                doc_feature[cur_func['func_name']] = new_feature
    doc_utils.dump_json(feature_file, doc_feature)
