from ..lib import keyword_utils

class Causal_analyzer:
    def __init__(self):
        self.chkcond_stat = {}
        self.func_stat = {
            "pre.call": {},
            "post.call": {}
        }

    def __update_pre_func_stat(self, feature, time):
        causal_type = 'pre.call'
        for func in feature[causal_type]:
            # format => {'target': total_time, ...}
            if func in self.func_stat[causal_type]:
                self.func_stat[causal_type][func] += time
            else:
                self.func_stat[causal_type][func] = time
        return

    def __update_post_func_stat(self, feature, time, chkval_cond="default"):
        causal_type = 'post.call'
        for func in feature[causal_type]:
            # format => {'target': [total_time, {'value': time, ...}], ...}
            if func in self.func_stat[causal_type]:
                if chkval_cond not in self.func_stat[causal_type][func][1]:
                    self.func_stat[causal_type][func][1][chkval_cond] = time
                else:
                    self.func_stat[causal_type][func][1][chkval_cond] += time
                self.func_stat[causal_type][func][0] += time
            else:
                self.func_stat[causal_type][func] = [time, {}]
                self.func_stat[causal_type][func][1][chkval_cond] = time
        return

    def update(self, feature, time):
        if 'causality' not in feature:
            return
        chkval_cond = get_chkval_cond(feature)
        if chkval_cond not in self.chkcond_stat:
            self.chkcond_stat[chkval_cond] = time
        else:
            self.chkcond_stat[chkval_cond] += time
        causal_feature = feature['causality']
        self.__update_pre_func_stat(causal_feature, time)
        self.__update_post_func_stat(causal_feature, time, chkval_cond)
        return

    def __causal_enhance(self, target, causal_func, causal_type):
        score = 0
        if causal_type == "pre.call":
            pass
        else:
            if keyword_utils.is_pre(target) and keyword_utils.is_post(causal_func):
                score = 0.3
            elif keyword_utils.is_post(causal_func):
                score = 0.1
        return score

    def __post_causal_cond(self, func_stat, threshold):
        causal_cond = {}
        valid_len = len(func_stat[1])
        # Ignore the 'no_check' stat
        if "no_check" in func_stat[1]:
            score = func_stat[1]['no_check']/self.chkcond_stat['no_check']
            if valid_len == 1 or score > threshold:
                causal_cond['no_check'] = round(score, 3)
            valid_len -= 1
        for chk_cond in func_stat[1]:
            if chk_cond == "no_check":
                continue
            score = func_stat[1][chk_cond]/self.chkcond_stat[chk_cond]
            if valid_len == 1 or score > threshold:
                causal_cond[chk_cond] = round(score, 3)
        return causal_cond

    def __filter_casual_functions(self, target, causal_type, sum_time, threshold):
        causal_funcs = {}
        for causal_func, stat in self.func_stat[causal_type].items():
            # Ignore to enhance for the small codebase(num(traces) <= 50).
            enhanced_score = self.__causal_enhance(target, causal_func, causal_type) \
                                if sum_time >= 50 else 0
            cur_time = stat if type(stat) == int else stat[0]
            score = cur_time/sum_time + enhanced_score
            if score >= threshold:
                if causal_type == "post.call":
                    # For post.call, format: [score, casual_cond]
                    # causal_cond: {"cond": score, ...}
                    causal_cond = self.__post_causal_cond(stat, threshold) # - enhanced_score
                    causal_funcs[causal_func] = [round(score, 3), causal_cond]
                else:
                    # For pre.call, format: {func: score, ...}
                    causal_funcs[causal_func] = round(score, 3)
            if cur_time/sum_time < 0.2:
                break
        if causal_type == "post.call":
            causal_funcs = dict(sorted(causal_funcs.items(),
                            key=lambda kv: (kv[1][0], kv[0]), reverse=True))
        else:
            causal_funcs = dict(sorted(causal_funcs.items(),
                            key=lambda kv: (kv[1], kv[0]), reverse=True))
        return causal_funcs

    def get_specification(self, func_name, sum_time, threshold, disable_code):
        if disable_code:
            return {
                "pre_functions": {},
                'post_functions': {},
            }

        # Normally:
        self.func_stat['pre.call'] = dict(sorted(self.func_stat['pre.call'].items(),
                                        key=lambda kv: (kv[1], kv[0]), reverse=True))
        self.func_stat['post.call'] = dict(sorted(self.func_stat['post.call'].items(),
                                        key=lambda kv: (kv[1][0], kv[0]), reverse=True))
        pre_causal_funcs = self.__filter_casual_functions(func_name, 'pre.call', sum_time, threshold)
        post_causal_funcs = self.__filter_casual_functions(func_name, 'post.call', sum_time, threshold)
        return {
            "pre_functions": pre_causal_funcs,
            'post_functions': post_causal_funcs,
        }


def get_chkval_cond(feature):
    if 'retval' not in feature:
        return "defalut"
    retchk_feature = feature['retval']['check']
    if retchk_feature['checked']:
        if retchk_feature['compared_with_non_const']:
            chkval_cond = "non_const"
        else:
            chkval_cond = str(retchk_feature['compared_with_const']) + \
                            '_' + retchk_feature['check_cond']
    elif retchk_feature['indir_checked']:
        chkval_cond = "indir_chk"
    else:
        chkval_cond = "no_check"
    return chkval_cond
