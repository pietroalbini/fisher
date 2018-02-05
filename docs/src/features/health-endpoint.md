# Monitoring with the `/health` endpoint

While [status hooks](status-hooks.md) allow you to record what happened after
the execution of your scripts, they can't be used to monitor what's happening
*right now* on your Fisher instance. If you need to know that though (for
example to have graphs on your favourite monitoring solution) the `/health`
HTTP endpoint provides an easy way to retrieve that data.

## API reference

The endpoint can be accessed with a GET HTTP request to the `/health` URL. The
endpoint returns a JSON response, with the following schema (keep in mind new
fields can be added in future Fisher releases):

```
{
    "result": {
        "busy_threads": 2,
        "max_threads": 2,
        "queued_jobs": 42
    },
    "status": "ok"
}
```

The `status` field returns if the request was successful: it can be `ok` if
there is some data available or `forbidden` if the endpoint is disabled in the
configuration. The returned data is contained in the `result` field, and
contains:

* `busy_threads`: the number of threads currently processing webhooks
* `max_threads`: the number of threads allocated to processing webhooks
* `queued_jobs`: the number of jobs waiting to be processed in the queue

## Configuration

If you don't plan to use the endpoint on your instance, you can disable it in
the [configuration file](../docs/config.md). This won't affect the performance
at all, but avoids exposing the information to the outside world. When
disabled, the endpoint returns a 403 HTTP status code when called, and contains
`forbidden` in the `status` field of the returned JSON.

To disable the endpoint, set the `http.health-endpoint` configuration to `false`:

```
[http]
health-endpoint = false
```
