# The `GitHub` provider

The GitHub provider allows you to integrate with [GitHub](https://github.com),
a popular code hosting platform. GitHub supports a wide range of different
webhooks, spanning from code pushes to comments.

The provider performs some consistency checks on the incoming webhooks, to
ensure they come from GitHub. It also ignores incoming pings from GitHub (such
as the ones sent when the webhook is created), so the script will be executed
only when something really happens.

If you need to ensure no one can send fake webhooks, you can configure GitHub
to sign all outgoing webhooks with a secret key you provide: if you put it in
the configuration comment the provider will reject every incoming webhook with
an invalid signature.

## Configuration

```plain
## Fisher-GitHub: {"secret": "secret key", "events": ["push", "pull_request"]}
```

The provider is configured with a [configuration
comment](../config-comments.md), and supports the following keys:

* `secret`: the secret key used to sign webhooks
* `events`: a whitelist of GitHub events you want to accept

## Environment variables

The provider sets the following environment variables during the execution of
the script:

* `FISHER_GITHUB_EVENT`: the name of the event of this webhook
* `FISHER_GITHUB_DELIVERY_ID`: the ID of the webhook delivery
