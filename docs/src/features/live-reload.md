# Live reloading

One of the design decisions of Fisher is to rely on an in-memory queue to store
all the jobs waiting to be processed. While this allows Fisher to be faster
(and avoids the need to configure an external database), having no persistency
means if the Fisher process is restarted all the jobs in the queue will be
lost.

Fisher needs to be restarted from time to time though, mainly if you need to
add new scripts or the configuration has to be changed. In order to avoid
losing the queue in those cases, Fisher has support for reloading the scripts
list and its configuration at runtime, without restarting the process.

## Reloading with signals

In order to reload Fisher, you need to send a `SIGUSR1` to the main Fisher
process: if you don't use any process manager on your machine, a simple
`killall` should do the trick:

```
$ killall -USR1 fisher
```

This command will reload **all** the Fisher instances the current user has the
rights to send signals to: if you have multiple instances running it might be
best to get the right PID and send the signal only to that one (with the `kill`
command).

## What happens when you reload a Fisher instance

When you tell a Fisher instance to reload, multiple things happens to ensure
everything is reloaded properly. Here is a list of all the steps. Please note
there are no guarantees this list will be accurate in past or future releases.

* First of all, the Fisher instance is locked, to prevent errors with new jobs
  coming in while updating the configuration: this means no new queued jobs
  will be processed, and the webhook endpoint will reply with *503 Unavailable*.

* Then, if any setting in the `[http]` configuration section is changed, the
  entire internal HTTP server is restarted.

* Then, if any of the other configuration entries is changed, their value is
  updated.

* Then, all the scripts will be reloaded from disk.

* Finally, the Fisher instance is unlocked, even if the reload fails.
