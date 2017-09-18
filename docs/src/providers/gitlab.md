## The `GitLab` provider

The GitLab provider allows you to integrate with
[GitLab](https://about.gitlab.com), a code hosting platform also available
self-hosted. GitLab supports a few webhooks, mostly related to its main
features.

The provider perform some consistency checks on the incoming webhooks, to
ensure they come from a GitLab instance. It can also check if the secret key
sent by GitLab along with the webhook matches the one configured in the script
(using configuration comments), rejecting invalid requests.

## Configuration

```plain
## Fisher-GitLab: {"secret": "secret key", "events": ["Push", "Issue"]}
```

The provider is configured with a [configuration
comment](../config-comments.md), and supports the following keys:

* `secret`: the secret key used to sign webhooks
* `events`: a whitelist of GitLab events you want to accept

## Environment varialbles

The provider sets the following environment variables during the execution of
the script:

* `FISHER_GITLAB_EVENT`: the name of the event of this webhook
