pub fn translate(input: &str) -> String {
    // Bash uses [[ ]] for conditionals, standard POSIX uses [ ]
    let mut translated = input.replace("[[", "[");
    translated = translated.replace("]]", "]");

    // Bash allows 'function my_func()', standard shells just use 'my_func()'
    translated = translated.replace("function ", "");

    translated
}