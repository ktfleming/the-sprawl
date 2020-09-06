#!/bin/bash

set -e

# Background color lightened by 2%
START=373343
TARGET=$1

pastel gradient --colorspace RGB $START $TARGET | pastel format hex |
  sed -e 's/#//' | # remove hash sign that we don't need
  sed -e 's/\(..\)\(..\)\(..\)/\[0x\1, 0x\2, 0x\3\],/' # put in [u8; 3] format
