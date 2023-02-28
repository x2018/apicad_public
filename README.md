APICAD is for detecting three common types (return check, argument check, and causal calls) of API misuse bugs.

In APICAD, there are two parallel workflows. The first is for analyzing code, while the second is for analyzing documents. Note that `Workflow 1` is indispensable since we at least need to analyze the code to be detected.

# Prerequisites

- Rust (https://rustup.rs)

- Clang, LLVM, wllvm (https://github.com/travitch/whole-program-llvm)

  > Note: The default supported version of LLVM is 12.0.0, if we use a different version of LLVM, we may need to change the version of llvm-sys which is dependent by llir (had been placed in "./src/analyzer/llir-0.2.2". For more details, see https://crates.io/crates/llir).

- Python3 environment (developed on python 3.8)

  In addition to [the standard library](https://docs.python.org/3.8/library/index.html), it also depends on requests, lxml, and hanlp.

  - Hanlp (https://hanlp.hankcs.com/docs/)

# Installation

- install prerequisites

  ```bash
  # Note: assume we have proper permissions to execute the below commands.
  # Otherwise, we may meet the errors about "permission denied".
  $ apt-get -y install clang-12 python3 python3-pip curl
  $ ln -s /usr/bin/clang++-12 /usr/bin/clang++ && \
  	ln -s /usr/bin/clang-12 /usr/bin/clang && \
  	ln -s /usr/bin/clang-cpp-12 /usr/bin/clang-cpp && \
  	ln -s /usr/bin/llvm-link-12 /usr/bin/llvm-link && \
  	ln -s /usr/bin/llvm-ar-12 /usr/bin/llvm-ar
  $ pip3 install --upgrade wllvm requests lxml hanlp
  # One option may need to be confirmed during the process:
  $ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  # Restart the shell to reload PATH environment variable.
  # Or, configure for the current shell:
  $ source "%HOME/.cargo/env"
  ```

- After installing the above prerequisites, we can use the following instructions to init the analyzing environment of APICAD (in the root directory of APICAD):

  ```bash
  # Build the Rust binary
  $ make
  
  # Set environment variables of APICAD for the current shell
  $ source setup.sh
  # or we can set environment variables for all (/usr/bin)
  # `make remove-bin` is to remove them from /usr/bin
  $ make install
  ```

You can also refer to the build command in `RUN` of [dockerfile](./docker/Dockerfile) or directly use Docker to build APICAD:

```sh
$ docker build -t apicad_docker -f docker/Dockerfile .
$ docker run -it apicad_docker
```

# Workflow 1

In workflow 1, we can run with 2 steps to generate code-based spec:

1. build (and generate .bc files):

   ```sh
   # compile the source code
   $ apicad build [config|make|cmake...]
   
   # (Optional) generate .bc files for the complied projects
   # If this operation is not performed in advance,
   # apicad will attempt to automatically do this in `apicad analyze`
   $ apicad generate-bc [-obj --bcdir=...]
   # Or we can directly use clang to generate the bitcode file
   $ clang -c -emit-llvm source.c -g -o source.bc
   ```

2. generate symbolic traces and features:

   ```sh
   # generate symbolic traces and features for all functions
   # note: analyze for a specific funciton by "--target-fn=..."
   $ apicad analyze [--bcdir=...] [--outdir=...]
   ```

   There are other options such as `--target`, `--bc` etc, take `--help` to see their details.

   > Note: This part is developed based on the symbolic engine provided by [ARBITRAR](https://github.com/petablox/arbitrar/).

Then we can detect API misuse bugs:

```sh
$ apicad detect [--type retval|arg.pre|arg.post|causality] [--target func_name]
```

# Workflow 2

For more accurate detection, APICAD can give play to both code-based spec and doc-based spec rather than taking a single source.

To enable doc-based spec for detection, we need to collect documents and then use our document analyzer to generate doc-based spec.

1. collect documents from the website

   ```sh
   # Now only support to handle these three documents
   $ apicad doc-collect [--target glibc|linux|openssl]
   ```

   For other documents, we need to write additional code to collect and preprocess them into the required format.

2. analyze documents to generate doc-based spec

   ```sh
   $ apicad doc-analyze [--semantic-type return|args|causality]
   ```

Then we can detect API misuse bugs with doc-enhanced spec by adding the option `--enable-doc`:

```sh
$ apicad detect --enable-doc [--type retval|arg.pre|arg.post|causality] [--target func_name]
```

# Note

The paper "APICAD: Augmenting API Misuse Detection through Specifications from Code and Documents" related to this project has been accepted to appear at ICSE 2023. This repository only contains the prototype code. The prepared artifact, which provides the tool prototype and original results as well as the guidance and information needed to use the tool, build the evaluation and replicate/reproduce the main experimental results reported in the paper, is archived on Software Heritage at https://archive.softwareheritage.org/browse/origin/https://github.com/apicad1/artifact. The corresponding repository on GitHub is https://github.com/apicad1/artifact.

