# Introduction to providers

If you want to integrate your scripts with third-party websites and services,
providers are the way to go. They allow you to filter only the requests coming
from them, and they also give your scripts more information specific to that
provider.

Fisher has currently native support for these providers:

* [Standalone](standalone.md) - for scripts not tied to a specific third-party
  website
* [GitHub](github.md) - for webhooks coming from
  [GitHub.com](https://github.com)
* [GitLab](gitlab.md) - for webhooks coming from a
  [GitLab](https://about.gitlab.com) instance

## Applying a provider to a script

In order to apply a provider to a script, you need to add its [configuration
comment](../config-comments.md) to the top of the script. After Fisher is
started/reloaded, it will start filtering requests according to that provider.
You can also add multiple providers to a single script, and they will be
validated according to the ordering they're wrote in the script.
