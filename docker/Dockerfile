FROM ubuntu:20.04

RUN apt-get update && \
    apt-get -y install \
    clang-12 python3 python3-pip curl && \
    ln -s /usr/bin/clang++-12 /usr/bin/clang++ && \
	ln -s /usr/bin/clang-12 /usr/bin/clang && \
	ln -s /usr/bin/clang-cpp-12 /usr/bin/clang-cpp && \
	ln -s /usr/bin/llvm-link-12 /usr/bin/llvm-link && \
	ln -s /usr/bin/llvm-ar-12 /usr/bin/llvm-ar

RUN pip3 install --upgrade wllvm requests lxml hanlp
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rust.sh && \
    bash rust.sh -y
# Note: In China, you may need to use mirror websites like below:
# RUN pip3 install --upgrade wllvm requests lxml hanlp -i https://pypi.tuna.tsinghua.edu.cn/simple/
# RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rust.sh && \
#     RUSTUP_UPDATE_ROOT="https://mirrors.ustc.edu.cn/rust-static/rustup" \
#     RUSTUP_DIST_SERVER="https://mirrors.tuna.tsinghua.edu.cn/rustup" \
#     bash rust.sh -y

WORKDIR /apicad
COPY src /apicad/src
COPY bin /apicad/bin
COPY Makefile /apicad/Makefile
COPY setup.sh /apicad/setup.sh
COPY apicad /apicad/apicad
