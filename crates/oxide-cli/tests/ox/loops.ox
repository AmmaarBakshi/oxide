#!/usr/bin/env oxide
# Loops: for and while, with break and continue.

for fruit in apple banana cherry {
    echo "fruit: $fruit"
}

i=0
while [ $i -lt 5 ] {
    i=$(( i + 1 ))
    if [ $i -eq 2 ] { continue }
    if [ $i -eq 4 ] { break }
    echo "i = $i"
}

total=0
for n in {1..5} {
    total=$(( total + n ))
}
echo "sum of 1..5 = $total"

for row in 1 2 {
    for col in a b {
        printf '%s%s,' $row $col
    }
    echo
}
