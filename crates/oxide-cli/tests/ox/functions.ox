#!/usr/bin/env oxide
# Functions: arguments, locals, return statuses and recursion.

greet() {
    echo "hello, $1"
}

function count_args {
    echo "$# args: $*"
}

factorial() {
    if [ $1 -le 1 ] {
        echo 1
        return
    }
    local rest=$(factorial $(( $1 - 1 )))
    echo $(( $1 * rest ))
}

is_even() {
    return $(( $1 % 2 ))
}

greet world
count_args a "b c" d
echo "5! = $(factorial 5)"

for n in 3 4 {
    if is_even $n { echo "$n is even" } else { echo "$n is odd" }
}

counter=0
bump() {
    counter=$(( counter + 1 ))
}
bump; bump; bump
echo "counter = $counter"
