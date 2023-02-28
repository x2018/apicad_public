#!/usr/bin/env python3
import os, subprocess
import re, requests, gzip, tarfile
from ..lib import utils


def capture_lasted_manfile(url):
    response = requests.get(url)
    filtered_link = re.findall("man-pages-[0-9,.]*tar.gz", response.text)
    lasted_link = url + filtered_link[-1]
    return lasted_link


''' decompress .tar '''
def un_tar(file_name, dir_path, display=False):
    tar = tarfile.open(file_name)
    files = tar.getnames()
    man_dir = os.path.join(dir_path, files[0])
    for file in files:
        if display:
            print(file)
        tar.extract(file, dir_path)
    tar.close()
    if not os.path.isdir(man_dir):
        man_dir = os.path.dirname(man_dir)
    return man_dir


# doc_dir - the storage directory of data
def handle_glibc(doc_dir):
    print("===============================================")
    print("====           Handling glibc info        =====")
    dir = os.path.join(doc_dir, "glibc")
    utils.mkdir(dir)

    # "Download the Linux manual documantation from \
    #  https://www.kernel.org/doc/man-pages/download.html. \
    #  Decompress it, make install-man3 to {dir}")
    download_page = "https://mirrors.edge.kernel.org/pub/linux/docs/man-pages/"
    lasted_manfile_link = capture_lasted_manfile(download_page)
    response = requests.get(lasted_manfile_link)
    tar_file = os.path.join(dir, "man.tar")
    with open(tar_file, "wb") as f:
        f.write(gzip.decompress(response.content))
    man_path = os.path.join(dir, un_tar(tar_file, dir))
    cmd = ["make", "install-man3", f"prefix={dir}"]
    ret = subprocess.run(cmd, cwd=man_path, stderr=subprocess.STDOUT)
    # The decompressed files should at dir/share/man/man3/*
    if ret.returncode != 0:
        raise Exception(f"Failure during running {cmd}")

    print("===============================================")
