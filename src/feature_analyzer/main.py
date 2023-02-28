import json
from .casual_analyzer import Causal_analyzer
from .arg_pre_analyzer import Argpre_analyzer
from .arg_post_analyzer import Argpost_analyzer
from .ret_analyzer import Ret_analyzer

def get_threshold(sum_time, threshold=None, rho=None):
    if threshold != None and type(threshold) == float:
        if threshold > 0 and threshold < 1:
            return threshold
        else: # feedback to user?
            pass
    else: # maybe internal error?
        pass
    e = 2.718281828459045
    rho = 500 if rho == None else rho
    if rho == 0:
        return 0.8
    return 0.5 + 0.3/(1 + pow(e, -(sum_time-rho)/(rho/5)))

### specification_map ###
def get_specification_map(features_map, func_name, threshold, rho, disable_code):
    specification_map = {'causal': {}, 'arg.pre': {}, 'arg.post': {}, 'ret': {}}
    sum_time = sum(features_map['time'])
    if sum_time == 0:
        return specification_map
    threshold = get_threshold(sum_time, threshold, rho)
    # init the state of feature analyzers
    casual_analyzer = Causal_analyzer()
    argpre_analyzer = Argpre_analyzer()
    argpost_analyzer = Argpost_analyzer()
    ret_analyzer = Ret_analyzer()

    for i, cur_feature in enumerate(features_map['feature']):
        cur_time = features_map['time'][i]
        # causal sub specification
        casual_analyzer.update(cur_feature, cur_time)
        # arg pre sub specification
        argpre_analyzer.update(cur_feature, cur_time)
        # arg post sub specification
        argpost_analyzer.update(cur_feature, cur_time)
        # ret sub specification
        ret_analyzer.update(func_name, cur_feature, cur_time)

    specification_map['info'] = {'threshold': threshold, 'traces_num': sum_time}
    specification_map['causal'] = casual_analyzer.get_specification(func_name, sum_time, threshold, disable_code)
    specification_map['arg.post'] = argpost_analyzer.get_specification(sum_time, threshold, disable_code)
    specification_map['arg.pre'] = argpre_analyzer.get_specification(sum_time, threshold, disable_code)
    specification_map['ret'] = ret_analyzer.get_specification(sum_time, threshold, argpre_analyzer.arg_num, disable_code)
    return specification_map


''' features_map
{
    'func_name': {
        'time': [3,2,4], 
        'feature': [feature1,feature2,feature3], 
        'loc': [[locs_1],[locs_2],[locs_3],...]
    },...
}
'''
def get_features_map_new(feature_paths, remove_dup=False):
    features = {'time': [], 'feature': [], 'loc': []}
    for file in feature_paths:
        with open(file, "r") as f:
            try:
                new_feature = json.load(f)
                loc = new_feature['loc']
                # skip the feature which does not record location
                if loc == "":
                    continue
                new_feature.pop('loc')
                if new_feature not in features['feature']:
                    features['time'].append(1)
                    features['feature'].append(new_feature)
                    features['loc'].append([loc])
                else:
                    index = features['feature'].index(new_feature)
                    if loc not in features['loc'][index]:
                        features['loc'][index] += [loc]
                        features['time'][index] += 1
                    # By default, it is counted even if it is at the same location
                    # If remove_dup is set, the same feature at a location will be counted only once
                    elif not remove_dup:
                        features['time'][index] += 1
            except:
                print(f"Can't parse {file}")
    return features


def features_analyze(feature_paths, func_name, remove_dup=False, threshold=None, rho=None, disable_code=False):
    features_map = get_features_map_new(feature_paths, remove_dup)
    specification_map = get_specification_map(features_map, func_name, threshold, rho, disable_code)
    return features_map, specification_map
