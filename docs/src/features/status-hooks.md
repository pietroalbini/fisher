# Monitoring with status hooks

Fisher doesn't have any monitoring or reporting solution built-in, for a simple
reason: there are countless ways you could do that, and integrating all of them
would be a daunting task. Instead, Fisher provides **status hooks**, a way to
build the integration yourself in a simple way.

[Check out the tutorial for an hands-on introduction.][tutorial]

[tutorial]: ../tutorial/failure-email.md

## Status hooks execution

Status hooks are executed when an event happens inside of Fisher, allowing you
to react to it. The following events are supported:

* `job-completed`: a job completed without any error
* `job-failed`: a job failed to execute, probably due to an error

Status hooks are executed in the scheduler along with the normal jobs, but with
a priority of `1000`. This means they will be executed before any other job,
but you can override this behavior by giving the most important scripts an
higher priority.

## Creating status hooks

To create a status hook, you just need to create a script that uses the
`Status` provider:

```plain
## Fisher-Status: {"events": ["job-failed"], "scripts": ["hook1.sh"]}
```

The status hook is configured with a [configuration
comment](../docs/config-comments.md), and supports the following keys:

* `events`: the list of events you want to catch
* `scripts`: execute the status hook only for these hooks *(optional)*

## Execution environment

Status hooks are executed with the following environment variables:

* `FISHER_STATUS_EVENT`: the name of the current event
* `FISHER_STATUS_SCRIPT_NAME`: the name of the script that triggered the event
* `FISHER_STATUS_SUCCESS`: `0` if the script failed, or `1` if it completed
* `FISHER_STATUS_EXIT_CODE`: the script exit code (if it wasn't killed)
* `FISHER_STATUS_SIGNAL`: the signal that killed the script (if it was killed)
* `FISHER_STATUS_STDOUT`: path to the file containing the stdout of the script
* `FISHER_STATUS_STDERR`: path to the file containing the stderr of the script
