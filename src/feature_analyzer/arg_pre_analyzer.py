class Argpre_analyzer:
    def __init__(self):
        self.arg_num = -1
        self.args_checked_count = []
    
    def __init_state(self):
        for _ in range(self.arg_num):
            self.args_checked_count.append(0)
        return

    def update(self, feature, time):
        if 'arg.pre' not in feature:
            return

        argpre_feature = feature['arg.pre']
        # Init default self.arg_num
        if self.arg_num == -1:
            self.arg_num = argpre_feature['arg_num']
            self.__init_state()
        # Lazily handle the functions with variable arguments
        elif self.arg_num == 0 or argpre_feature['arg_num'] != self.arg_num:
            return

        for num in range(self.arg_num):
            cur_feature = argpre_feature['feature'][num]
            if cur_feature['check']['checked']:
                self.args_checked_count[num] += time
        return

    def get_specification(self, sum_time, threshold, disable_code):
        if disable_code:
            return {
                'args_need_to_check': [[False, None] for _ in self.args_checked_count]
            }

        # Normally:
        # format: {'args_need_to_check': [[true/false, score], ...]}
        args_need_to_check = [[i/sum_time >= threshold, round(i/sum_time, 3)] 
                                for i in self.args_checked_count]
        return {
            "args_need_to_check": args_need_to_check,
        }
