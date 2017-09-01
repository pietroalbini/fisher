# The fisher command

The `fisher` command allows you to start an instance of Fisher. It acceps a few
options, and the directory containing the scripts as its only argument:

```
$ fisher /path/to/scripts
```

## Available options

### --behind-proxies COUNT

Tell Fisher it's running behind one or more proxy. If the proxy adds the
`X-Forwarded-For` HTTP header, Fisher will be able to determine the real user
IP address, which might be used during the validation of incoming webhooks.

You need to provide the exact number of proxies between the user and the Fisher
instance as the argument of the flag. For example, if you configuration
includes nginx as a reverse proxy between the user and Fisher, you should set
the value to `1`.

### -b IP:PORT, --bind IP:PORT

Bind the Fisher internal webserver to a different IP address and port.
Examples:

- Listening for local requests on port `8000`: `--bind 127.0.0.1:8000`
- Listening for every request on port `8080`: `--bind 0.0.0.0:8080`

By default Fisher listens on `127.0.0.1:8000`.

### -j COUNT, --jobs COUNT

Set `COUNT` as the maximum number of parallel jobs executed concurrently by
Fisher. By default this value is set to `1`, but if you increase it the
scheduler will start executing jobs in parallel, whenever possible.

### --no-health

Disable the `/health` endpoint, returning a HTTP 403 response every time
someone tries to access that page. You should use this option if you don't need
the information returned by that endpoint and you don't want to leak it to the
world either.

### -r, --recursive

Look for scripts not only in the directory provided as the first argument, but
also in its subdirectories. This allows you to organize your scripts in a
better way.
