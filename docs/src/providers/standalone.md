# The `Standalone` provider

The standalone provider is probably the most useful one, because it can be used
without a third-party website and also to integrate with an external website
not directly supported by Fisher.

This provider validates if the incoming requests have a secret value in them
(either in the query string param `secret` or the header `X-Fisher-Secret`). If
they don't have them they will be rejected. Both the query string argument name
and the header name are configurable on a per-script basis.

This provider also supports whitelisting the IP addresses allowed to call the
webhook. This way you can provide a basic level of authorization without
sharing secret keys around.

This provider doesn't provide any environment variable to the executing script.

## Configuration

```
## Fisher-Standalone: {"secret": "secret key", "from": ["127.0.0.1"]}
```

The provider is configured with a [configuration
comment](../docs/config-comments.md), and supports the following keys:

* `from` *(optional)*: a list of IP addresses to whitelist
* `secret` *(optional)*: the secret key the request must contain
* `param_name` *(optional)*: the custom name of the query string param
  containing the secret key
* `header_name` *(optional)*: the custom name of the header containing the
  secret key
