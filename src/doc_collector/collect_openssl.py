#!/usr/bin/env python3
import os
from ..lib import utils

'''
desc: get infomation of openssl
    - NAME/SYNOPSIS/DESCRIPTION/RETURN VALUES/NOTES/BUGS/SEE ALSO
'''
# doc_dir - the storage directory of data
def handle_openssl(doc_dir):
    print("===============================================")
    print("====         Handling openssl info        =====")
    print("====          From official source        =====")
    dir = os.path.join(doc_dir, "openssl")
    utils.mkdir(dir)

    print("Please step into the root directory of Openssl, "\
            f"and then copy doc/man3/*.pod to {dir}. \n" \
            "Or you can crawl from website: https://www.openssl.org/docs/manmaster/man3/")

    print("===============================================")
