# xxx: Consider more complex situations?
class Argpost_analyzer:
    def __init__(self):
        self.arg_num = -1
        self.args_checked_count = []

    def update(self, feature, time):
        if 'arg.post' not in feature:
            return
        argpost_feature = feature['arg.post']
        # Init default self.arg_num
        if self.arg_num == -1:
            self.arg_num = argpost_feature['arg_num']
            for _ in range(self.arg_num):
                self.args_checked_count.append(0)
        # Lazily handle the functions with variable arguments
        elif self.arg_num == 0 or argpost_feature['arg_num'] != self.arg_num:
            return
        for num in range(self.arg_num):
            cur_feature = argpost_feature['feature'][num]
            is_not_constant = True
            # Only focus on the argument which is not a constant
            if 'arg.pre' in feature:
                is_not_constant = not feature['arg.pre']['feature'][num]['is_constant']
            if is_not_constant and cur_feature['used_in_check']:
                self.args_checked_count[num] += time
        return

    def get_specification(self, sum_time, threshold, disable_code):
        if disable_code:
            return {
                'args_need_to_check': [[False, None] for _ in self.args_checked_count]
            }

        # Normally:
        # format: {'args_need_to_check': [[true/false, score], ...]}
        return {
            'args_need_to_check': [[i/sum_time >= threshold, round(i/sum_time, 3)] \
                                    for i in self.args_checked_count]
        }
