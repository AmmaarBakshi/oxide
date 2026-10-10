#!/usr/bin/env oxide
# case: literal words, alternatives, globs and a default arm.

describe() {
    case $1 in
        start|begin) echo "$1: starting" ;;
        stop) echo "$1: stopping" ;;
        *.txt) echo "$1: a text file" ;;
        [0-9]*) echo "$1: a number" ;;
        *) echo "$1: unknown" ;;
    esac
}

for word in start begin stop notes.txt 42 other {
    describe $word
}

answer=yes
case $answer in
    y|yes)
        echo agreed
        ;;
    n|no)
        echo declined
        ;;
esac
