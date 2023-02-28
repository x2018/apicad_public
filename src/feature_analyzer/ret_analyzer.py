# xxx: Judge which condition is for failure?
# Expect Output: ret_need_to_check & feasible_chkvals
class Ret_analyzer:
    def __init__(self):
        self.checked_count = 0 # checked
        self.cur_checked = 0 # checked and the same function is in post.call
        self.has_same_in_post = 0 # the number of trace which has the same function in post.call
        self.chkvals_stat = {'time': [], 'chkvals': []}

    def update(self, func_name, feature, time):
        chkval = None
        if 'retval' not in feature:
            return
        causal_post = feature['causality']['post.call'] if 'causality' in feature else {}
        has_same_in_post = func_name in causal_post
        if has_same_in_post:
            self.has_same_in_post += time
        ret_feature = feature['retval']
        if ret_feature['check']['checked']:
            self.checked_count += time
            if has_same_in_post:
                self.cur_checked += time
            if ret_feature['check']['compared_with_non_const']:
                chkval = "non_const"
            # Consider the equivalent check value
            elif ret_feature['check']['check_cond'] in ["gt", "le"]:
                    chkval = ret_feature['check']['compared_with_const'] + 1/4
            elif ret_feature['check']['check_cond'] in ["ge", "lt"]:
                chkval = ret_feature['check']['compared_with_const'] - 1/4
            else:
                chkval = ret_feature['check']['compared_with_const']
        elif ret_feature['check']['indir_checked']:
            chkval = "indir_chk"
            self.checked_count += time
            if has_same_in_post:
                self.cur_checked += time
        if chkval != None:
            if chkval not in self.chkvals_stat['chkvals']:
                self.chkvals_stat['chkvals'].append(chkval)
                self.chkvals_stat['time'].append(time)
            else:
                index = self.chkvals_stat['chkvals'].index(chkval)
                self.chkvals_stat['time'][index] += time
        return

    def get_specification(self, sum_time, threshold, arg_num, disable_code):
        # format: {'need_to_check': [true/false, score], 'invalid_chkvals': {chkval: score, ...}}
        retval_need_to_check = [False, round(self.checked_count/sum_time, 3)]
        feasible_chkvals = {}
        no_need_to_check_if_same_in_post = False
        no_same_in_post_need_to_check = False
        # Lazily enable this functionality if there can be only one same argument.
        if arg_num == 1:
            no_need_to_check_if_same_in_post = False if self.has_same_in_post == 0 else self.cur_checked/self.has_same_in_post < threshold
            if sum_time > self.has_same_in_post:
                no_same_in_post_need_to_check = (self.checked_count-self.cur_checked)/(sum_time-self.has_same_in_post) > threshold
        if self.checked_count/sum_time >= threshold:
            retval_need_to_check[0] = True
        for i, chkval in enumerate(self.chkvals_stat['chkvals']):
            if self.chkvals_stat['time'][i]/self.checked_count >= 1/len(self.chkvals_stat['time']):
                feasible_chkvals[chkval] = round(self.chkvals_stat['time'][i]/self.checked_count, 3)

        if disable_code:
            return {'need_to_check': [False, 0],
                'valid_chkvals': [],
                # Just regard it as an empirical filtering rule instead of inherent specs
                'no_need_to_check_if_same_in_post': no_need_to_check_if_same_in_post,
                'no_same_in_post_need_to_check': False
            }
        else:
            return {
                'need_to_check': retval_need_to_check,
                'valid_chkvals': feasible_chkvals,
                'no_need_to_check_if_same_in_post': no_need_to_check_if_same_in_post,
                'no_same_in_post_need_to_check': no_same_in_post_need_to_check
            }
