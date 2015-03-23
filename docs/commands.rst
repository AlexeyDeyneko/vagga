.. _commands:

========
Commands
========


Every commands in ``vagga.yaml`` is mapping with a tag that denotes command
type. The are two command types ``!Command`` and ``!Supervise`` illustrated
by the following example:

.. code-block:: yaml

    containers: {ubuntu: ... }
    commands:
      bash: !Command
        description: Run bash shell inside the container
        container: ubuntu
        run: /bin/bash
      download: !Supervise
        description: Download two files simultaneously
        children:
          amd64: !Command
            container: ubuntu
            run: wget http://cdimage.ubuntu.com/ubuntu-core/trusty/daily/current/trusty-core-amd64.tar.gz
          i386: !Command
            container: ubuntu
            run: wget http://cdimage.ubuntu.com/ubuntu-core/trusty/daily/current/trusty-core-i386.tar.gz


Common Parameters
=================

These parameters work for both kinds of commands:


``description``
    Description that is printed in when vagga is runned without arguments

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


Parameters of `!Command`
========================

``container``
    The container to run command in

``run``
    The command to run. It's either a string (which is passed to
    ``/bin/sh -c``) or a list of command and arguments.

``work-dir``
    The working directory to run in. Path relative to project root. By
    default command is run in the same directory where vagga started (sans
    the it's mounted as ``/work`` so the output of ``pwd`` would seem to be
    different)

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

``user-id``
    The user id to run command as. If the ``external-user-id`` is omitted this
    has same effect like using ``sudo -u`` inside container (except it's user
    id instead of user name)

``external-user-id``
    (experimental) This option allows to map the ``user-id`` as seen by
    command itself to some other user id inside container namespace (the
    namespace which is used to build container). To make things a little less
    confusing, the following two configuration lines::

        user-id: 1
        external-user-id: 0

    Will make your command run as user id 1 visible inside the container
    (which is "daemon" or "bin" depending on distribution). But outside the
    container it will be visible as your user (i.e. user running vagga). Which
    effectively means you can create/modify files in project directory without
    permission errors, but ``tar`` and other commands which have different
    behaviour when running as root would think they are not root (but has
    user id 1)


Parameters of `!Supervise`
==========================

``mode``
    The set of processes to supervise and mode. See :ref:`supervision` for more
    info

``children``
    A mapping of name to child definition of children to run. All children are
    started simultaneously.
