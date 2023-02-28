#!/usr/bin/env python3
from . import utils
import logging

logger = logging.getLogger(utils.color_str("BUGREPORT"))


def bug_report_dict(report_list):
    # 'file_dir': report_list[0],
    report_dict = {
        'func_name': report_list[0],
        'feature': report_list[1],
        'frequency': report_list[2],
        'type': report_list[3],
        "alarm_text": report_list[4],
        'doc_sentences': report_list[5],
        'doc_feature': report_list[6],
    }
    # 'file_dir': report_list[7],}
    return report_dict


# resorted bug reports format
'''
file_dir, 
{   
    "location": [{func_name, feature, frequency, type, alarm_text, doc_sentences, doc_feature}, {...}, ...],
    ...
}
'''
def bug_report(log_path, bug_reports, only_report_locations=False):
    utils.config_log_file(logger, log_path, "w")
    resoted_reports = resort_reports(bug_reports)
    for loc in resoted_reports:
        reports_info = resoted_reports[loc]
        length = len(reports_info)
        # Only report locations and basic bug types
        if only_report_locations:
            output_log = f"{reports_info[0]['func_name']}: {loc}, TYPE:"
            bug_types = []
            for i in range(length):
                if {reports_info[i]['type']} not in bug_types:
                    output_log += f" {reports_info[i]['type']}"
                    bug_types.append({reports_info[i]['type']})
            logger.error(output_log)
            continue
        # Normal report
        report_str = f"{reports_info[0]['func_name']}:\n"
        report_str += f"\tLocation:{loc}\n"
        reported_feature = []
        for i in range(length):
            if reports_info[i]['feature'] in reported_feature:
                continue
            reported_feature.append(reports_info[i]['feature'])
            report_str += f"\tTYPE: {reports_info[i]['type']}. " + \
                f"feature: {reports_info[i]['feature']}\n" + \
                f"\tViolation: {reports_info[i]['alarm_text']}\n"
            # report_str += f"\tdoc-realated: {reports_info[i]['doc_sentences']}. " + \
            #     f"Violation: {reports_info[i]['doc_feature']}\n"
            # report_str += f"\ttrace frequency: {reports_info[i]['frequency']}\n"
            # report_str += f"\tdir: {reports_info[i]['file_dir']}\n"
            report_str += "\n" if length > 1 else ""

        logger.error(report_str)

    logger.info(f"Total reports: {len(resoted_reports)}")


# resort bug reports by locations
# original bug reports format
'''
{   "retval": {
        "location": [{func_name, feature, frequency, type, doc_sentences, doc_feature}, {...}, ...],
        ...
    },
    ...
}
'''
def resort_reports(bug_reports):
    resorted_reports = {}
    for feature_type in bug_reports:
        for loc in bug_reports[feature_type]:
            if loc not in resorted_reports:
                resorted_reports[loc] = bug_reports[feature_type][loc]
            else:
                resorted_reports[loc] += bug_reports[feature_type][loc]
    return resorted_reports
