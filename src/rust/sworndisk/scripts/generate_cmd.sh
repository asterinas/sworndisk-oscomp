#!/bin/bash
# a shell script to generate .module_name.o.cmd

CWD=`pwd`
CRATE_NAME="dm_sworndisk"
MODULE_NAME="dm-sworndisk"
MODULE_PATH="${CWD}/${MODULE_NAME}.o"
OUTPUT_DIR="${CWD}/target/target/release/deps"
DEP_FILE_NAME=`ls $OUTPUT_DIR | grep "${CRATE_NAME}.*[\.d]$"`
DEP_FILE_PATH=${OUTPUT_DIR}/${DEP_FILE_NAME}
OBJ_FILE_NAME=`ls $OUTPUT_DIR | grep "${CRATE_NAME}.*[\.o]$"`
DEP=$(cat $DEP_FILE_PATH | grep $OBJ_FILE_NAME | cut -d ' ' -f 2-)


printf "cmd_$MODULE_PATH := RUST_MODFILE=${CWD}/${MODULE_NAME} cargo build

source_$MODULE_PATH := ${CWD}/${MODULE_NAME}/src/lib.rs

deps_$MODULE_PATH := ${DEP}

${MODULE_PATH} := \$(deps_${MODULE_PATH})

\$(deps_${MODULE_PATH}):
" > ".${MODULE_NAME}.o.cmd"