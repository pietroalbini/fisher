# Configuration comments

*Configuration comments* are special comments, located at the top of the
scripts, used by Fisher to determine the configuration of the script itself.

All the configuration comments are optional, but if you want to take advantage
of some Fisher features you have to use them.

## Syntax

Configuration comments must be located at the top of the file, before any
empty line. This means they can be located after the `#!shebang` or other
comments at the top of the file.

They must start with `##`, and are composed of a key and a JSON value. For
example, this is a valid configuration comment:

```
## Fisher: {"parallel": false}
```

## The `Fisher` configuration comment

The `Fisher` configuration comment allows you to configure the behavior of
Fisher for this specific script. Its value must be valid JSON.

```
## Fisher: {"parallel": false, "priority": 10}
```

### `priority`

The priority of the script. Scripts with higher priority will always be
executed before scripts with lower priority, even if the lower priority ones
are first in the queue.

Status hooks have a default priority of `1000`: if you choose a priority higher
than that, be advised that if you have a lot of scripts in the queue the
execution of status hooks might be delayed, or they might not be executed at
all.

It must be a signed integer, and its default value is `0`.

### `parallel`

This configuration key tells Fisher if the script can be executed in parallel.

Fisher can support executing multiple scripts at the same time to work through
the queue faster, but not every script might support it. For example, if a
script runs a database migration you don't want to execute two of them at the
same time.

With this configuration key you can tell the scheduler this script doesn't
support being executed in parallel and the scheduler will avoid doing that,
while continuing to executing the other ones in parallel.

It must be a boolean, and its default value is `true`.
