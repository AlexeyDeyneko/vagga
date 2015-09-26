.. highlight:: bash

============
Command Line
============

When runnin ``vagga``, it  finds the ``vagga.yaml`` or ``.vagga/vagga.yaml``
file in current working directory or any of its parents and uses that as a
project root directory.

When running ``vagga`` without arguments it displays a short summary of which
commands are defined by ``vagga.yaml``, like this::

    $ vagga
    Available commands:
        run                 Run mysample project
        build-docs          Build documentation using sphinx

Refer to :ref:`commands` for more information of how to define commands for
vagga.

There are also builtin commands. All builtin commands start with underscore
``_`` character to be clearly distinguished from user-defined commands.

Builtin Commands
================

All commands have ``--help``, so we don't duplicate all command-line flags
here

* ``vagga _run CONTAINER CMD ARG...`` -- run arbitrary command in container
  defined in vagga.yaml
* ``vagga _build CONTAINER`` -- builds container without running a command
* ``vagga _clean`` -- removes images and temporary files created by vagga. To
  fully remove ``.vagga`` directory you can run ``vagga _clean --everything``.
  For other operations see ``vagga _clean --help``
* ``vagga _list`` -- list of commands (including builtin ones when using
  ``--builtin`` flag)
* ``vagga _version_hash`` -- prints version hash for the container, might be
  used in some automation scripts


Normal Commands
===============

If :ref:`command<commands>` declared as ``!Command`` you get a command
with the following usage::

    Usage:
        vagga [OPTIONS] some_command [ARGS ...]

    Runs a command in container, optionally builds container if that does not
    exists or outdated. Run `vagga` without arguments to see the list of
    commands.

    positional arguments:
      some_command          Your defined command
      args                  Arguments for the command

    optional arguments:
      -h,--help             show this help message and exit
      -E,--env,--environ NAME=VALUE
                            Set environment variable for running command
      -e,--use-env VAR      Propagate variable VAR into command environment
      --no-build            Do not build container even if it is out of date.
                            Return error code 29 if it's out of date.
      --no-version-check    Do not run versioning code, just pick whatever
                            container version with the name was run last (or
                            actually whatever is symlinked under
                            `.vagga/container_name`). Implies `--no-build`

All the  ``ARGS`` that follow command are passed to the command even if they
start with dash ``-``.


Supervise Commands
==================

If :ref:`command<commands>` declared as ``!Supervise`` you get a command
with the following usage::


    Usage:
        vagga run [OPTIONS]

    Run full server stack

    optional arguments:
      -h,--help             show this help message and exit
      --only PROCESS_NAME [...]
                            Only run specified processes
      --exclude PROCESS_NAME [...]
                            Don't run specified processes
      --no-build            Do not build container even if it is out of date.
                            Return error code 29 if it's out of date.
      --no-version-check    Do not run versioning code, just pick whatever
                            container version with the name was run last (or
                            actually whatever is symlinked under
                            `.vagga/container_name`). Implies `--no-build`

Currently there is no way to provide additional arguments to commands declared
with ``!Supervise``.

The ``--only`` and ``--exclude`` arguments are useful for isolating some
single app to a separate console. For example, if you have ``vagga run``
that runs full application stack including a database, cache, web-server
and your little django application, you might do the following::

    $ vagga run --exclude django

Then in another console::

    $ vagga run --only django

Now you have just a django app that you can observe logs from and restart
independently of other applications.
