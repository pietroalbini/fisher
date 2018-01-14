# Tutorial: send emails when scripts fails

This isn't a perfect world, and even the best scripts can fail: a way to know
when one of them doesn't execute correctly and why it failed is invaluable.

Fisher doesn't have a ready to be used solution for this though: there are
countless way you can be notified about a problem, and supporting all of them
would be an impossible task. Instead, Fisher provides you a simple but powerful
interface to script how notifications are sent: [status
hooks](../features/status-hooks.md).

A status hook is a normal script Fisher starts every time a job completed its
execution, that receives all the availabe information about the job execution.
This means it can then report those information to your monitoring software of
choice, or notify you about what happened the way you like.

In this tutorial we're going to create a status hook that sends an email with
the script output if an execution fails, so you're alerted when something goes
wrong and why.

## Installing dependencies

Since we're going to send out emails when something bad happens, the `mail`
command has to be installed on the server running Fisher. How to do so depends
on which operative system you have installed, but in Debian/Ubuntu you can
install the tool with the command:

```plain
$ sudo apt install mailutils
```

## Creating the status hook

First of all, you need to create the script that Fisher will call when a job
fails. To do so, create an executable file in the directory loaded by Fisher
with this content:

```bash
#!/bin/bash
## Fisher-Status: {"events": ["job-failed"]}
```

All the status hooks needs to use the special [Status
provider](../features/status-hooks.md), which allows to filter the events the
status hooks will handle. In this case, our status hook will handle just the
`job_failed` event.

Then we can edit the script to send the scary email to our address:

```bash
#!/bin/bash
## Fisher-Status: {"events": ["job-failed"]}

NOTIFY_ADDRESS="bob@example.com"

echo "A Fisher script failed." | mail -s "A script failed" "${NOTIFY_ADDRESS}"
```

## Retrieving the job details

The script we just wrote works fine, but doesn't tell you anything other than
"a script failed". In order to be really useful, the email needs to contain
more details about the execution. Fisher provides all the available information
through the environment, so you can retrieve them in every scripting language
you use. Check out the [status hooks
documentation](../features/status-hooks.md) for a list of all the environment
variables.

Let's change the script to be more useful then:

```bash
#!/bin/bash
## Fisher-Status: {"events": ["job-failed"]}

NOTIFY_ADDRESS="bob@example.com"

echo "Script ${FISHER_STATUS_SCRIPT_NAME} failed to execute!" > m
echo >> m
echo "Host:               $(hostname)" >> m
echo "Exit code:          ${FISHER_STATUS_EXIT_CODE:-none}" >> m
echo "Killed with signal: ${FISHER_STATUS_SIGNAL:-none}" >> m
echo >> m
echo "Standard output" >> m
echo "===============" >> m
cat "${FISHER_STATUS_STDOUT}" >> m
echo >> m
echo "Standard error" >> m
echo "==============" >> m
cat "${FISHER_STATUS_STDERR}" >> m

cat m | mail -s "Script ${FISHER_STATUS_SCRIPT_NAME} failed" "${NOTIFY_ADDRESS}"
```

Now you'll know everything when something goes wrong!
