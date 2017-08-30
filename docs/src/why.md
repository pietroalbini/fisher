# Why you should use Fisher

There's a lot of ways to catch and act on incoming webhooks out there, such
as simple CGI scripts, small dedicated web applications or even other projects
similar to Fisher. Fisher has multiple advantages over them, though.

## Simple to operate

Fisher is made to be simple to operate and monitor.

There is no need to create a configuration file listing all the available
webhooks, or to use a specific naming convention for the scripts in the
filesystem: you just need to put all the scripts in a directory, and Fisher
will automatically scan them and load all the *executable* files as new
webhooks.

Because there is no centralized configuration file, each script is configured
by adding comments to the top of its source file: this allows you to keep all
the scripts along with their configuration in source control. For example, to
create a webhook that receives `push` events from GitHub you can write this
script:

```bash
#!/bin/bash
# Fisher-GitHub: {"events": ["push"]}

echo "I'm the script!"
```

## Support for multiple third-party providers

Fisher supports a wide range of third-party providers you can use to validate
if incoming webhooks are actually valid. This means Fisher is not tied to a
single external service like other projects, and you can even use it in a
standalone way.

You can add one or multiple providers to each script, and when a webhook
arrives, its request is checked against all of them. This allows a single
script to be triggered by both GitHub and GitLab, for example.

## Clean scripts execution context

After a webhook is received and validated, Fisher executes the related script
in a clean environment, to avoid strange errors caused by different
environments.

All the scripts are executed in a temporary directory deleted after the
execution, which is set also as the `$HOME` to avoid dotfiles being written
somewhere else. This allows, for example, to execute multiple instances of the
same script in parallel.

To guarantee reproducibility, the execution environment is stripped out of most
of the existing environment variables: only a few of them are kept by default,
and you can manually add custom ones if you need them.

## Advanced scheduling

Fisher includes a scheduler to execute the validated webhooks. It's
deterministic, and it has multiple features to control the order in which the
scripts are executed.

One of those features is scripts priority: you can specify what's the priority
of each script, and higher-priority scripts in the queue will be executed
before the others. This ensures the most important webhooks will be processed
right away!

Fisher has also the ability to execute multiple scripts in parallel, to work
through the queue faster. Unfortunately, not every script can be parallelized
(think about a deploy script executing some migrations in the database). To
avoid this the scheduler allows to mark single scripts as "non parallel", and
it will never schedule that script multiple times, while running the other ones
in parallel.

## Easy monitoring

Fisher can be easily monitored, to give you all the diagnostics information you
need.

There is an endpoint, `/health`, which you can use to do black-box monitoring:
it returns a few numbers (such as the number of webhooks in the queue), that you
can use to build graphs or trigger alerts after they reach a certain threshold.

If you need insights why a webhook failed, you can also create *status hooks*,
special scripts executed after a script is run. Status hooks receive all the
details about the previous execution, such as standard output/error and exit
code. You can use them to log jobs into your existing systems, or to alert you
when something fails.
