#!/bin/bash

DIR=$(cd `dirname $BASH_SOURCE`; pwd)

if [[ ! $PATH =~ $DIR/bin ]]
then
    echo "=== adding $DIR/bin to PATH ==="
    export PATH="$DIR/bin:$PATH"
fi
