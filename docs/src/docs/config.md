# The configuration file

Fisher uses a configuration file to store its settings. The configuration file
uses the TOML syntax (a serialization format similar to INI files), and has
sensible defaults for every option (so you can omit the ones you don't want to
change).

This page contains a description for each available setting. You can also find
a commented configuration file distributed with all the official packages, and
in [the source code repository][src].

[src]: https://github.com/pietroalbini/fisher/blob/master/config-example.toml

-----

## `[http]` section

The `[http]` section contains the configuration for the built-in HTTP server
and API.

### `behind-proxies`

The number of proxies Fisher sits behind. This is used to correctly parse the
X-Forwarded-For HTTP header in order to retrieve the correct origin IP. If this
value is zero, the header is ignored, otherwise it must be present with the
correct number of entries to avoid requests being rejected.

**Type**: integer - **Default**: `0`

### `bind`

The network address Fisher will listen on. By default, only requests coming
from the local machine are accepted (thus requiring a reverse proxy in front of
the instance). If you want to expose Fisher directly on the Internet you should
change the IP address to `0.0.0.0`.

**Type**: string - **Default**: `127.0.0.1:8000`

### `health-endpoint`

If this is set to false, the `/health` HTTP endpoint (used to monitor the
instance) is disabled. Disable this if you don't need monitoring and you don't
want the data to be publicly accessible.

**Type**: boolean - **Default**: `true`

### `rate-limit`

Rate limit for failed requests (allowed requests / time period). The rate limit
only applies to webhooks that failed validation, so it doesn't impact legit
requests (while keeping brute force attempts away). [Check out the rate limits
documentation](../features/rate-limits.md).

**Type**: string - **Default**: `10/1m`

-----

## `[scripts]` section

The `[scripts]` section configures how Fisher looks for scripts in the
filesystem.

### `path`

The directory containing all the scripts Fisher will use. Scripts needs to be
executable in order to be called.

**Type**: string - **Default**: `/srv/fisher-scripts`

### `recursive`

If this is set to true, scripts in subdirectories of `scripts.path` will also
be loaded, including from symlinks (be sure to check permissions before
changing this option).

**Type**: boolean - **Default**: `false`

-----

## `[jobs]` section

The `[jobs]` section configures how Fisher runs jobs (for example incoming
hooks).

### `threads`

Maximum number of parallel jobs you want to run.

**Type**: integer - **Default**: `1`

-----

## `[env]` section

Extra environment variables provided to the scripts Fisher starts. Since the
outside environment is filtered, this is the place to add every variable you
want to have available. You can add environment variables by adding extra
key-value pairs under this section, for example:

```toml
[env]
VAR_1 = "value"
VAR_2 = "1"
```
