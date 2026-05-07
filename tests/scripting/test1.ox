export STATUS=2

if [ $STATUS -eq 1 ] {
    echo "Status is One"
} elif [ $STATUS -eq 2 ] {
    echo "Status is Two"
} else {
    echo "Status is Unknown"
}