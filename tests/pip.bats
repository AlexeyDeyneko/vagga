setup() {
    cd /work/tests/pip
}

@test "py2: ubuntu pkg" {
    run vagga _run py2-ubuntu urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py2-ubuntu)
    [[ $link = ".roots/py2-ubuntu.b6bc38d1/root" ]]
}

@test "py2: alpine pkg" {
    run vagga _run py2-alpine urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py2-alpine)
    [[ $link = ".roots/py2-alpine.a7327653/root" ]]
}

@test "py2: ubuntu git" {
    run vagga _run py2-git-ubuntu urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py2-git-ubuntu)
    [[ $link = ".roots/py2-git-ubuntu.aedb2403/root" ]]
}

@test "py2: alpine git" {
    run vagga _run py2-git-alpine urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py2-git-alpine)
    [[ $link = ".roots/py2-git-alpine.569f9a5e/root" ]]
}

@test "py3: ubuntu pkg" {
    run vagga _run py3-ubuntu urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py3-ubuntu)
    [[ $link = ".roots/py3-ubuntu.c2f5a64e/root" ]]
}

@test "py3: ubuntu git" {
    run vagga _run py3-git-ubuntu urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py3-git-ubuntu)
    [[ $link = ".roots/py3-git-ubuntu.453926f2/root" ]]
}

@test "py2: ubuntu req.txt" {
    run vagga _run py2req-ubuntu urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py2req-ubuntu)
    [[ $link = ".roots/py2req-ubuntu.1730f1da/root" ]]
}

@test "py2: alpine req.txt" {
    run vagga _run py2req-alpine urp -Q key=val http://example.com
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    [[ ${lines[${#lines[@]}-1]} = http://example.com?key=val ]]
    link=$(readlink .vagga/py2req-alpine)
    [[ $link = ".roots/py2req-alpine.eb8c5b79/root" ]]
}

@test "py3: ubuntu req-https.txt" {
    run vagga _build py3req-https-ubuntu
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    link=$(readlink .vagga/py3req-https-ubuntu)
    [[ $link = ".roots/py3req-https-ubuntu.ce4dc161/root" ]]
}

@test "py3: alpine req-https.txt" {
    run vagga _build py3req-https-alpine
    printf "%s\n" "${lines[@]}"
    [[ $status = 0 ]]
    link=$(readlink .vagga/py3req-https-alpine)
    [[ $link = ".roots/py3req-https-alpine.356eb50e/root" ]]
}
