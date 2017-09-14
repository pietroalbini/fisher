# Scripts execution context

Fisher executes all the scripts in a clean environment, to remove every
chance of misbehaving on different machines. Here is described everything
guaranteed about the environment.

## Working directory

Scripts are executed in a temporary directory created by Fisher: this allows
you to download and build things without worrying about cleaning up after the
execution of your script.

This directory is usually located in `/tmp`, and it has a random name. It's
also set as the current working directory when the script is started, and as
the `$HOME`. Being the home directory means most of the dotfiles and caches
created during the execution are cleared out after the build.

## Environment variables

Fisher provides only a subset of environment variables to the processes.

### System environment variables

Most of the environment variables provided by the system are removed by Fisher.
A few of them are not, though: you can change them being assured all the
changes will be available to the scripts.

- `$LC_ALL` and `$LANG`: the system language
- `$PATH`: the system path used to search binaries

Also, those system environment variables are overridden by Fisher:

- `$HOME`: this is set to the build directory
- `$USER`: this is set to the current user name

### Fisher environment variables

Fisher adds its own environment variables to the mix. These variables allows
you to get more information about the incoming request:

- `$FISHER_REQUEST_IP`: the IP address of the client that sent the webhook
- `$FISHER_REQUEST_BODY`: the path to the file containing the raw request body

Other than these variable, each provider can add its own environment variables.
Check out the documentation for the providers you're using to learn more about
that.
