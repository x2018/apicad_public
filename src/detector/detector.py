import logging
from ..lib import bug_report

logger = logging.getLogger("detector")

class Detector:
    ''' bug reports format
    {
        "location": [{func_name, feature, trace_frequency, type, doc_sentences, doc_feature}, [...], ...],
        ...
    }
    '''
    def __init__(self, code_handler=None, doc_handler=None, log_path=None, code_feature_paths=None, check_type=None):
        self.code_handler = code_handler
        self.doc_handler = doc_handler
        self.log_path = log_path
        self.code_feature_paths = code_feature_paths
        self.bug_reports = {'retval': {}, 'arg.pre': {}, 'arg.post': {}, 'causality': {}}
        self.toleration_types = ['retval']  # , 'causality'
        self.toleration = {}
        if check_type == None:
            self.check_types = ['retval', 'arg.pre', 'arg.post', 'causality']
        else:
            self.check_types = [check_type]

    def init_toleration_list(self):
        for check_type in self.toleration_types:
            self.toleration[check_type] = []

    # (result, alarm_text). True means error status
    def __filter(self, check_type, func_name, complete_feature):
        if check_type:
            return self.code_handler.detection(check_type, func_name, complete_feature, self.doc_handler)
        else:
            pass
        return False, ""

    ''' 
    input: (location, bug_report_list = [func_name, feature, trace_frequency, feature_type, alarm_text, doc_related, doc_feature])
    '''
    def __update_bug_report(self, locs, feature_type, bug_report_list):
        for loc in locs:
            if feature_type not in self.toleration_types or loc not in self.toleration[feature_type]:
                if loc not in self.bug_reports[feature_type]:
                    self.bug_reports[feature_type][loc] = [bug_report.bug_report_dict(bug_report_list)]
                elif bug_report.bug_report_dict(bug_report_list) not in self.bug_reports[feature_type][loc]:
                    self.bug_reports[feature_type][loc] += [bug_report.bug_report_dict(bug_report_list)]

    def __check(self, frequency, feature_type, func_name, locations, complete_feature):
        is_bug, alarm_text = self.__filter(feature_type, func_name, complete_feature)
        if is_bug and alarm_text != "":
            self.__update_bug_report(
                locations, feature_type,
                [func_name, complete_feature[feature_type], frequency, feature_type, alarm_text, None, None])
        elif alarm_text == "":
            for location in locations:
                if feature_type in self.toleration_types and location not in self.toleration[feature_type]:
                    self.toleration[feature_type].append(location)
                # Tolerate if the location has correct trace when feature type is in toleration type list.
                if location in self.bug_reports[feature_type] and feature_type in self.toleration_types:
                    self.bug_reports[feature_type].pop(location)

    def __check_func_features(self, func_name):
        feature_map = self.code_handler.features_map[func_name]
        total_times = sum(feature_map['time'])
        for i, feature in enumerate(feature_map['feature']):
            loc = feature_map['loc'][i]
            time = feature_map['time'][i]
            frequency = round(time/total_times, 3)
            if loc == []: # may be internal error
                continue
            for check_type in self.check_types:
                if check_type not in feature:
                    continue
                ''' xxx: Only perform the `retval` check for functions that may have variable length arguments,
                since now we cannot handle all arguments properly and these functions usually do not have the
                responsibility for pre-/post- conditions. '''
                if self.code_handler.is_var_arg and check_type != "retval":
                    continue
                self.__check(frequency, check_type, func_name, loc, feature)

    ''' code_feature_map format
    {
        'func_name': {
            'retval': {
                    'time': [3,2,4], 
                    'feature': [feature1,feature2,feature3], 
                    'loc': [[locs_1],[locs_2],[locs_3],...]
            },...
        }
    }
    '''
    ''' doc_features format
    {
        'func_name': {
            'ret': {'value': [...], 'cond': [...]}
            ...
        }
    }
    '''
    def detect(self, only_report_locations=False):
        # {"func_name": [feature_paths]... }
        for func_name in self.code_feature_paths:
            logger.info(f"Processing {func_name}")
            # init toleration list for every function
            self.init_toleration_list()
            feature_paths = self.code_feature_paths[func_name]
            self.code_handler.init_item(func_name, feature_paths)
            if self.doc_handler != None:
                self.doc_handler.display(func_name)
            self.__check_func_features(func_name)
            self.code_handler.pop_item(func_name)

        bug_report.bug_report(self.log_path, self.bug_reports, only_report_locations)
