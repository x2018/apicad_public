#!/usr/bin/env python3
from ..lib.doc_utils import load_json
from ..lib.keyword_utils import is_variable_argument
from ..feature_analyzer.main import features_analyze
from . import retval_checker, causality_checker, arg_pre_checker, arg_post_checker


############################################## Doc_feature_handler ##############################################
class Doc_feature_handler:
    '''
    {
        'func_name': {
            'ret': {'value': [...], 'cond': [...]}
            ...
        }
    }
    '''
    def __init__(self, features_path, display_spec=False):
        self.display_spec = display_spec
        self.types = {'retval': 'ret', 'arg.pre': 'args', 'arg.post': 'args', 'causality': 'causality'}
        self.features = load_json(features_path)

    def __find_variant(self, func_name):
        # TODO: xxx: make this totally automatical by leveraging additional analysis?
        variant_name = []
        prefix = func_name.split("_")[0]
        if prefix in ["OPENSSL", "SSL"]:
            variant_name.append(func_name.replace(prefix, "CRYPTO"))
        position = len(func_name)
        while position > 0:
            position -= 1
            if func_name[position] >= '0' and func_name[position] <= '9':
                continue
            else:
                break
        if position != len(func_name) - 1:
            variant_name.append(func_name[:position + 1])
        else:
            variant_name += [func_name + str(i) for i in range(10)]
            variant_name += [func_name + "32", func_name + "64"]
        return variant_name

    def retrieve(self, func_name, feature_type=""):
        if func_name not in self.features:
            variant_name = self.__find_variant(func_name)
            unfound_num = 0
            for name in variant_name:
                if name in self.features:
                    func_name = name
                    break
                else:
                    unfound_num += 1
            if unfound_num == len(variant_name):
                return {}
        if feature_type == "":
            return self.features[func_name]
        elif feature_type not in self.types:
            return {}
        cur_type = self.types[feature_type]
        if cur_type not in self.features[func_name]:
            return {}
        return self.features[func_name][cur_type]

    def display(self, func_name):
        if self.display_spec:
            print('#' * 2 + " specifications inferred from doc " + '#' * 2)
            print(self.retrieve(func_name))
            print('-' * 39 + '\n')
        return
#################################################################################################################


############################################# Code_feature_hanlder ##############################################
class Code_feature_hanlder:
    def __init__(self, remove_dup=False, display_spec=False, threshold=None, rho=None, disable_code=False):
        self.remove_dup = remove_dup
        self.display_spec = display_spec
        self.threshold = threshold
        self.rho = rho
        self.disable_code = disable_code
        self.features_map = {}
        self.specification_map = {}
        self.is_var_arg = False

    def init_item(self, func_name, feature_paths):
        self.features_map[func_name], self.specification_map[func_name] = \
                                    features_analyze(feature_paths, func_name, self.remove_dup,
                                                     self.threshold, self.rho, self.disable_code)
        self.is_var_arg = True if is_variable_argument(func_name) else False
        if self.display_spec:
            print('#' * 2 + " specifications inferred from code " + '#' * 2)
            for item in self.specification_map[func_name]:
                print("%8s" % item, self.specification_map[func_name][item])
            print('#' * 39 + '\n')

    def pop_item(self, func_name):
        if func_name in self.features_map:
            self.features_map.pop(func_name)
        if func_name in self.specification_map:
            self.specification_map.pop(func_name)

    # (result, alarm_text). True means error status
    def detection(self, check_type, func_name, complete_feature, doc_handler=None):
        if check_type == "arg.pre" and 'arg.pre' in self.specification_map[func_name]:
            doc_feature = {} if doc_handler == None \
                            else doc_handler.retrieve(func_name, check_type)
            sub_specification = self.specification_map[func_name]['arg.pre']
            return arg_pre_checker.only_code_check(func_name, sub_specification, complete_feature, doc_feature)
        elif check_type == "arg.post" and 'arg.post' in self.specification_map[func_name]:
            doc_feature = {} if doc_handler == None \
                            else doc_handler.retrieve(func_name, check_type)
            sub_specification = self.specification_map[func_name]['arg.post']
            return arg_post_checker.only_code_check(sub_specification, complete_feature, doc_feature)
        elif check_type == "causality" and 'causal' in self.specification_map[func_name]:
            doc_feature = {} if doc_handler == None \
                            else doc_handler.retrieve(func_name)
            sub_specification = self.specification_map[func_name]['causal']
            return causality_checker.check_causality(func_name, sub_specification, complete_feature, doc_feature)
        elif check_type == "retval" and 'ret' in self.specification_map[func_name]:
            doc_feature = {} if doc_handler == None \
                            else doc_handler.retrieve(func_name, check_type)
            sub_specification = self.specification_map[func_name]['ret']
            return retval_checker.check_retval(func_name, complete_feature, sub_specification, doc_feature)
        else:
            return False, ""
#################################################################################################################
