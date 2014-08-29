===================
Vagga Configuration
===================

Main vagga configration file is ``vagga.yaml`` it's usually in the root of the
project dir. It can also be in ``.vagga/vagga.yaml`` (but it's not recommended).

The ``vagga.yaml`` has three sections:

* ``containers`` -- description of the containers
* ``commands`` -- a set of commands defined for the project
* ``variants`` -- defines a set of variables that can be used to customize
  containers and commands

.. _containers:

Containers
==========

Example of one container defined:

.. code-block:: yaml

  containers:
    sphinx:
      builder: debian
      parameters:
        packages: python-sphinx make coreutils bash

The YAML above defines a container named ``sphinx``, which is built by
``debian`` builder and install debian package ``python-sphinx`` (along with
other three) inside the container.

Container parameters:

``default-command``
    This command is used when running ``vagga _run <container_name>``. Note
    that this command doesn't use ``command-wrapper``, so you may include that
    value explicitly

``command-wrapper``
    The wrapper script thats used to run anything inside container. For example
    setting the value to ``/usr/bin/env`` and running ``vagga _run cmd args``
    will actually run ``/usr/bin/env cmd args``. This may be either a string,
    which is treated as single command (e.g. no split by space), or a list.

``shell``
    The shell used to run commands with ``run`` key, and for ``vagga _run -S``.
    ``command-wrapper`` is not used for it. This may be either a string,
    which is treated as single command (e.g. no split by space), or a list.
    For usual shell must be ``[/bin/sh, -c]``.

``builder``, ``paramaters``
    Name of the builder to make container and a mapping with builder
    parameters. All parameter values are strings. See :ref:`builders` for more
    info

``environ-file``
    The file with environment definitions. Path inside the container. The file
    consists of line per value, where key and value delimited by equals ``=``
    sign. (Its similar to ``/etc/environment`` in ubuntu or ``EnvironmentFile``
    in systemd, but doesn't support commands quoting and line wrapping yet)

``environ``
    The mapping, that constitutes environment variables set in container. This
    overrides ``environ-file`` on value by value basis.

``banner``
    The message that is printed before running process(es). Useful for
    documenting command behavior.

``banner-delay``
    The seconds to sleep before printing banner. For example if commands run
    a web service, banner may provide a URL for accessing the service. The
    delay is used so that banner is printed after service startup messages not
    before.  Note that currently vagga sleeps this amount of seconds even
    if service is failed immediately.

``epilog``
    The message printed after command is run. It's printed only if command
    returned zero exit status. Useful to print further instructions, e.g. to
    display names of build artifacts produced by command.

``uids``
    List of ranges of user ids that need to be mapped when container runs.
    User must have some ranges in ``/etc/subuid`` to run this contiainer,
    and total size of all allowed ranges must be larger or equal to the sum of
    sizes of all ranges specified in ``uids`` parameter.  Currenlty vagga
    applies ranges found in ``/etc/subuid`` one by one until all ranges are
    satisfied. It's not always optimal or desirable, we will allow to customize
    mapping in later versions.

``gids``
    List of ranges of group ids that need to be mapped when container runs.
    User must have some ranges in ``/etc/subgid`` to run this contiainer,
    and total size of all allowed ranges must be larger or equal to the sum of
    sizes of all ranges specified in ``gids`` parameter.  Currenlty vagga
    applies ranges found in ``/etc/subgid`` one by one until all ranges are
    satisfied. It's not always optimal or desirable, we will allow to customize
    mapping in later versions.

.. _provision:

``provision``
    The command-line to be run to provision the container. It's run in
    container itself, but comparing to normal vagga containers this one has
    writeable root, so you can install something, or copy config to the system
    folder. The ``/work`` directory is also mounted in this container (it's
    currently mounted writeable, but this fact may change in future).

    The ``provision`` command is run by ``shell``. And this means that shell
    must already be installed in container.

    The ``provision`` command is run with same environment variables as a
    builder, so may know details of build process, but doesn't obey
    environment of the target execution (e.g. ``PATH`` is used from outer
    environment). It may be changed or fixed in future. At the end of the day,
    you shouldn't rely on environment variables, and should setup everything
    needed right in the script.

.. _commands:

Commands
========

Example of command defined:

.. code-block:: yaml

   commands:
     build-docs:
       description: Build vagga documentation using sphinx
       container: sphinx
       work-dir: docs
       command: make

The YAML above defines a command named ``build-docs``, which is run in
container named ``sphinx``, that is run in ``docs/`` sub dir of project, and
will run command ``make`` in container. So running::

    > vagga build-docs html

Builds html docs using sphinx inside a container.

Command parameters:

``container``
    The container to run command in

``command``
    The command to run. It's either a string (which is treated as executable)
    or a list or command and arguments. If ``wrapper-script`` is defined in
    container, it prefixes this command.

``run``
    The command to run using a shell. Prefixed by shell defined in container
    (usually ``/bin/sh -c``)

``supervise``, ``supervise-mode``
    The set of processes to supervise and mode. See :ref:`supervision` for more
    info

``accepts-arguments``
    Denotes whether command accepts additional arguments. Defaults to ``false``
    for shell commands, and ``true`` for regular commands.

``environ``
    The mapping of environment to pass to command. This overrides environment
    specified in container on value by value basis.

``inherit-environ``
    The list of variables that will be inherited from user environment, when
    running a command. These variables override both ``environ`` in command
    and container's environment only if is set in user environment (including
    set to empty string). Inheriting variables is in generally discouraged
    because this makes reproducing environment harder.

``description``
    Description that is printed in when vagga is runned without arguments

``pid1mode``
    This denotes what is run as pid 1 in container. It may be ``wait``,
    ``wait-all-children`` or ``exec``. The default ``wait`` is ok for most
    regular processes. See :ref:`pid1mode` for more info.

``write-mode``
    The parameter specifies how container's base file system is used. By
    default container is immutable (corresponds to the ``read-only`` value of
    the parameter), which means you can only write to the ``/tmp`` or
    to the ``/work`` (which is your project directory).

    Another option is ``transient-hard-link-copy``, which means that whenever
    command is run, create a copy of the container, consisting of hard-links to
    the original files, and remove the container after running command. Should
    be used with care as hard-linking doesn't prevent original files to be
    modified. Still very useful to try package installation in the system. Use
    ``vagga _build --force container_name`` to fix base container if that was
    modified.


.. _variants:

Variants
========

Variant definition look like:

.. code-block:: yaml

   variants:
     py:
       default: 2.7
       options:
       - 2.7
       - 3.4

This can then be used in container in the following way:

.. code-block:: yaml

   containers:
     python:
       builder: ubuntu
       parameters:
         packages: python@py@

Without parameters this will install python 2.7. But you can run python using
following command::

    > vagga _run --variant py=3.4 python python3

The actual commands might use ``-v`` or ``--variant`` flag. So testing code
in both python versions might be run like this::

    > vagga -v py=3.4 python3 && vagga -v py=2.7 python

You may change default version in local config by running::

    > vagga _setvariant py 3.4

This will store default variant in ``.vagga/settings.yaml``.


.. _YAML: http://yaml.org
