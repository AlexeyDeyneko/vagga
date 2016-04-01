.. default-domain:: vagga

.. _settings:

========
Settings
========


Global Settings
===============


Settings are searched for in one of the following files:

 * ``$HOME/.config/vagga/settings.yaml``
 * ``$HOME/.vagga/settings.yaml``
 * ``$HOME/.vagga.yaml``


Supported settings:


.. opt:: storage-dir

    Directory where to put images build by vagga. Usually they are stored in
    ``.vagga`` subdirectory of the project dir. It's mostly useful when the
    ``storage-dir`` points to a directory on a separate partition. Path may
    start with ``~/`` which means path is inside the user's home directory.

.. opt:: cache-dir

    Directory where to put cache files during the build. This is used to speed
    up the build process. By default cache is put into ``.vagga/.cache`` in
    project directory but this setting allows to have cache directory shared
    between multiple projects. Path may start with ``~/`` which means path is
    inside the user's home directory.

.. opt:: site-settings

    (experimental) The mapping of project paths to settings for this specific
    project.

.. opt:: proxy-env-vars

    Enable forwarding for proxy environment variables. Default ``true``.
    Environment variables currently that this setting influence currently:
    ``http_proxy``, ``https_proxy``, ``ftp_proxy``, ``all_proxy``,
    ``no_proxy``.

.. opt:: external-volumes

   A mapping of volume names to the directories inside the host file system.

   .. note:: The directories must exist even if unused in any ``vagga.yaml``.

   For example, here is how you might export home:

   .. code-block:: yaml

      external-volumes:
        home: /home/user

   Then in `vagga.yaml` you use it as follows (prepend with `/volumes`):

   .. code-block:: yaml

      volumes:
        /root: !BindRW /volumes/home

   See :ref:`volumes` for more info about defining mount points.

   .. warning::

      1. Usage of volume is usually a subject for filesystem permissions. I.e.
         your user becomes `root` inside the container, and many system users
         are not mapped (not present) in container at all. This means that
         mounting `/var/lib/mysql` or something like that is useless, unless
         you chown the directory

      2. Any vagga project may use the volume if it's defined in global
         config. You may specify the volume in :opt:`site-settings` if you
         care about security (and you should).


All project-local settings are also allowed here.


Project-Local Settings
======================

Project-local settings may be in the project dir in:

 * ``.vagga.settings.yaml``
 * ``.vagga/settings.yaml``

All project-local settings are also allowed in global config.

While settings can potentially be checked-in to version control it's advised
not to do so.

.. opt:: version-check

    If set to ``true`` (default) vagga will check if the container that is
    already built is up to date with config. If set to ``false`` vagga will
    use any container with same name already built. It's only useful for
    scripts for performance reasons or if you don't have internet and
    containers are not too outdated.

.. opt:: ubuntu-mirror

    Set to your preferred ubuntu mirror. By default it's
    ``mirror://mirrors.ubuntu.com/mirrors.txt`` which means mirror will be
    determined automatically. Note that it's different from default in ubuntu
    itself where ``http://archive.ubuntu.com/ubuntu/`` is the default.

.. opt:: alpine-mirror

    Set to your preferred alpine mirror. By default it's the random one is
    picked from the list.

    .. note:: Alpine package manager is used not only for building
       :step:`Alpine` distribution, but also internally for fetching tools that
       are outside of the container filesystem (for example to fetch ``git``
       for :step:`Git` or :step:`GitInstall` command(s))
