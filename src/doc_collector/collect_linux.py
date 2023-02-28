#!/usr/bin/env python3
import os
import requests
from lxml import etree
from ..lib import utils

'''
desc: save the data into a file
'''
def save_linux_data(file_name, data):
    with open(file_name, "w", encoding='utf-8') as f:
        f.write(data)


def replace_bad_character(ori_str):
    new_str = ori_str.replace("Â", "")
    new_str = new_str.replace("¶", "")
    new_str = new_str.replace("â", "'")
    new_str = new_str.replace("â", "'")  # ‘
    new_str = new_str.replace("â", "'")  # ’
    new_str = new_str.replace("â", "'")  # “
    new_str = new_str.replace("â", "'")  # ”
    new_str = new_str.replace("â", "...")  # …
    new_str = new_str.replace(" ", " ")  #
    return new_str


'''
desc: get infomation of linux
    get per-function infomation in Linux Core API page
        - basic description, Parameters, Description, Note
    concern: may have some absent situations
    using lxml to filter xpath
'''
def get_linux_info(content):
    info = ""
    sections = content.xpath("div[@class='section']")
    if len(sections) > 0:
        for section in sections:
            info += get_linux_info(section)
    elif len(content.xpath("section")) > 0:
        for section in content.xpath("section"):
            info += get_linux_info(section)
    else:
        for element in content:
            if "h" == element.tag[0]:
                continue
            if 'class' in element.attrib:
                if element.attrib['class'] in ['function', 'c function']:
                    info += "\n" + "=" * 20 + "\n"
                elif element.attrib['class'] == 'c macro':
                    continue
            if 'class' in element.attrib and element.attrib['class'] == 'simple':
                info += replace_bad_character(element.text) if element.text != None else ""
                for e in element:
                    info += replace_bad_character(e.xpath("string(.)")) + "\n"
            else:
                info += replace_bad_character(element.xpath("string(.)")) + "\n"
            if element.tag == "p":
                info += "\n"
    return info

'''
doc_dir - the storage directory of data
Now the default version is set to v5.15.
See the versions at https://www.kernel.org/doc/html/.
Note: This is an empirical implementation for the version <= v6.1, which
may not support the future version because the websites are updating.
'''
def handle_linux(doc_dir):
    print("===============================================")
    print("====          Handling linux info         =====")
    print("====       From linux Core API page       =====")
    dir = os.path.join(doc_dir, "linux")
    utils.mkdir(dir)
    source_linux = requests.get("https://www.kernel.org/doc/html/v5.15/core-api/kernel-api.html")
    parser = etree.HTMLParser(encoding='utf-8')
    html_elements = etree.HTML(source_linux.text, parser=parser).xpath("//*[@id='the-linux-kernel-api']")
    saved_data = ""
    for content in html_elements:
        saved_data += get_linux_info(content)
    source_linux = requests.get("https://www.kernel.org/doc/html/v5.15/core-api/mm-api.html")
    parser = etree.HTMLParser(encoding='utf-8')
    html_elements = etree.HTML(source_linux.text, parser=parser).xpath("//*[@id='memory-management-apis']")
    for content in html_elements:
        saved_data += get_linux_info(content)
    save_linux_data(os.path.join(dir, "linux_api.txt"), saved_data)
    print("===============================================")
