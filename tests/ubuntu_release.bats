setup() {
    cd /work/tests/ubuntu_release
}

@test "UbuntuRelease builds" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    vagga _build vivid
    link=$(readlink .vagga/vivid)
    [[ $link = ".roots/vivid.0fd62318/root" ]]
}

@test "Run echo command in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    run vagga echo-cmd hello
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ $output = hello ]]
    run vagga echo-cmd world
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ $output = world ]]
}

@test "Run echo shell in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    run vagga echo-shell
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ $output = "" ]]
    run vagga echo-shell hello
    printf "%s\n" "${lines[@]}"
    [[ $status = 122 ]]
    [[ $output =~ "Unexpected argument" ]]
}

@test "Run echo shell with arguments in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    run vagga echo-shell-arg
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ $output = "" ]]
    run vagga echo-shell-arg hello
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ $output = "hello" ]]
}

@test "Run absent command in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    run vagga test something
    printf "%s\n" "${lines[@]}"
    [[ $status -eq 121 ]]
    [[ $output =~ "Command test not found." ]]
}

@test "Run vivid bc in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    run vagga vivid-calc 100*24
    printf "%s\n" "${lines[@]}"
    [[ $status -eq 0 ]]
    [[ ${lines[${#lines[@]}-1]} = "2400" ]]
    link=$(readlink .vagga/vivid-calc)
    [[ $link = ".roots/vivid-calc.b9bec918/root" ]]
}

@test "ubuntu_release: Run vivid bc in ubuntu derived from release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    run vagga vivid-derived-calc 100*24
    printf "%s\n" "${lines[@]}"
    [[ $status -eq 0 ]]
    [[ ${lines[${#lines[@]}-1]} = "2400" ]]
    link=$(readlink .vagga/vivid-derive)
    [[ $link = ".roots/vivid-derive.b9bec918/root" ]]
}

@test "Run trusty bc in ubuntu release" {
    run vagga trusty-calc 23*7+3
    printf "%s\n" "${lines[@]}"
    [[ $status -eq 0 ]]
    [[ ${lines[${#lines[@]}-1]} = "164" ]]
    link=$(readlink .vagga/trusty-calc)
    [[ $link = ".roots/trusty-calc.5a774ae3/root" ]]
}

@test "Test VAGGAENV_* vars in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    VAGGAENV_TESTVAR=testvalue run vagga _run vivid printenv TESTVAR
    printf "%s\n" "${lines[@]}"
    [[ $status -eq 0 ]]
    [[ $output = testvalue ]]
}

@test "Test set env in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    run vagga --environ TESTVAR=1value1 _run vivid printenv TESTVAR
    printf "%s\n" "${lines[@]}"
    [[ $status -eq 0 ]]
    [[ $output = 1value1 ]]
}

@test "Test propagate env in ubuntu release" {
    skip "15.04 is absent on cdimage already, will be fixed by #230"
    TESTVAR=2value2 run vagga --use-env TESTVAR _run vivid printenv TESTVAR
    printf "%s\n" "${lines[@]}"
    [[ $status -eq 0 ]]
    [[ $output = 2value2 ]]
}
