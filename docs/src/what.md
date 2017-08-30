# What is Fisher

Fisher is a simple, lightweight webhooks receiver and processor. It allows you
to filter incoming webhooks, validate them and execute scripts based on which
hook was received.

## Why should I use Fisher

There are a lot of ways to catch and act on incoming webhooks out there, such
as simple CGI scripts, small dedicated web applications or even other projects
similar to Fisher. Fisher has multiple advantages over them, though.

### Simple to operate

Fisher is made to be simple to be operate and monitor.

There is no need to create a configuration file listing all the available
webhooks, or to use a specific naming convention for the scripts in the
filesystem: you just need to put all the scripts in a directory, and Fisher
will automatically scan it and load all the *executable* files as new webhooks.

Because there is no centralized configuration file, each script is configured
by adding comments to the top of its source file: this allows you to keep all
the scripts along with their configuration in source control. For example, to
create a webhook that receives `push` events from GitHub you can write this
script:

```bash
#!/bin/bash
## Fisher-GitHub: {"events": ["push"]}

echo "I'm the script!"
```

### Clean scripts execution context

After a webhook is received and validated, Fisher executes the related script
in a clean environment, to avoid strange errors caused by different
environments.

All the scripts are executed in a temporary directory deleted after the
execution, which is set also as the `$HOME` to avoid dotfiles being written
somewhere else. This allows, for example, to execute multiple instances of the
same script in parallel.

To guarantee reproducibility, the execution environment is cleaned of most of
the existing environment variables: only a few of them are keeped by default,
and you can manually add custom ones if you need them.
